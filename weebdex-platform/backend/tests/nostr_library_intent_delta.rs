//! Pure original-codec intent checks. No database, authority or HTTP write path.
use anyhow::{Result, ensure};
use manga_node::{
    db::Id,
    library_intent_delta::{DeltaPlan, Intent, MAX_AFFECTED_BUCKETS, MAX_INTENT_IDS},
    library_manifest::{Chunk, Descriptor, Family, FamilyRoot, Limits, Record, Root, bucket_for},
};
use std::collections::BTreeMap;
fn id(n: u64) -> Id {
    let mut id = [0; 32];
    id[..8].copy_from_slice(&n.to_be_bytes());
    id
}
fn chapter(n: u64, completed: bool, page: Option<u32>) -> Record {
    Record::Chapter {
        id: id(n),
        completed,
        page,
        updated_at: 1,
    }
}
fn graph(records: Vec<Record>, counts: [usize; 4]) -> Result<(Root, BTreeMap<Id, Vec<u8>>)> {
    let limits = Limits::default();
    let mut buckets = Family::ALL
        .into_iter()
        .zip(counts)
        .map(|(f, n)| (f, vec![vec![]; n]))
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let b = buckets.get_mut(&record.family()).unwrap();
        let i = bucket_for(&record.key(), b.len())?;
        b[i].push(record);
    }
    let mut objects = BTreeMap::new();
    let mut families = vec![];
    for family in Family::ALL {
        let mut chunks = vec![];
        for records in buckets.remove(&family).unwrap() {
            if records.is_empty() {
                chunks.push(Descriptor::empty());
            } else {
                let bytes = Chunk::new(family, records, &limits)?.encode(&limits)?;
                let (_, summary) = Chunk::decode(&bytes, &limits)?;
                chunks.push(Descriptor::from(&summary));
                objects.insert(summary.hash(), bytes);
            }
        }
        families.push(FamilyRoot { family, chunks });
    }
    let root = Root { families };
    Root::decode(&root.encode(&limits)?, &limits)?;
    Ok((root, objects))
}
fn patch(
    root: &Root,
    objects: &BTreeMap<Id, Vec<u8>>,
    family: Family,
    key: Id,
    mutate: impl FnOnce(&mut Vec<Record>),
) -> Result<(Root, BTreeMap<Id, Vec<u8>>)> {
    let mut new = root.clone();
    let f = new
        .families
        .iter_mut()
        .find(|f| f.family == family)
        .unwrap();
    let index = bucket_for(&key, f.chunks.len())?;
    let old = &f.chunks[index];
    let mut records = if old.is_empty() {
        vec![]
    } else {
        Chunk::decode(&objects[&old.hash], &Limits::default())?
            .0
            .records()
            .to_vec()
    };
    mutate(&mut records);
    let mut changed = BTreeMap::new();
    f.chunks[index] = if records.is_empty() {
        Descriptor::empty()
    } else {
        let bytes = Chunk::new(family, records, &Limits::default())?.encode(&Limits::default())?;
        let (_, summary) = Chunk::decode(&bytes, &Limits::default())?;
        changed.insert(summary.hash(), bytes);
        Descriptor::from(&summary)
    };
    Ok((new, changed))
}
fn validate(
    old: Option<&Root>,
    new: &Root,
    intent: &Intent,
    objects: &BTreeMap<Id, Vec<u8>>,
    changed: &BTreeMap<Id, Vec<u8>>,
) -> Result<manga_node::library_intent_delta::DeltaStats> {
    DeltaPlan::prepare(old, new, intent)?.validate(changed, |d| Ok(objects[&d.hash].clone()))
}
#[test]
fn requested_unchanged_bucket_is_checked_not_assumed_success() -> Result<()> {
    let (root, objects) = graph(vec![chapter(1, false, None)], [1; 4])?;
    let intent = Intent::read_markers(&[id(1)], &[])?;
    assert!(
        validate(Some(&root), &root, &intent, &objects, &BTreeMap::new()).is_err(),
        "unchanged false cannot receipt read=true"
    );
    let (root, objects) = graph(vec![chapter(1, true, None)], [1; 4])?;
    let stats = validate(Some(&root), &root, &intent, &objects, &BTreeMap::new())?;
    assert_eq!(stats.old_chunks_decoded, 1);
    assert_eq!(stats.new_chunks_decoded, 0);
    assert_eq!(stats.records_visited, 1);
    let (empty, objects) = graph(vec![], [1; 4])?;
    assert!(validate(None, &empty, &intent, &objects, &BTreeMap::new()).is_err());
    validate(
        None,
        &empty,
        &Intent::read_markers(&[], &[id(1)])?,
        &objects,
        &BTreeMap::new(),
    )?;
    Ok(())
}
#[test]
fn exact_read_unread_preserve_partial_pages_and_all_unrequested_records() -> Result<()> {
    let (root, objects) = graph(
        vec![chapter(1, false, Some(3)), chapter(2, true, None)],
        [1; 4],
    )?;
    let read = Intent::read_markers(&[id(1)], &[])?;
    let (new, changed) = patch(&root, &objects, Family::Chapters, id(1), |r| {
        if let Record::Chapter {
            completed,
            updated_at,
            ..
        } = &mut r[0]
        {
            *completed = true;
            *updated_at = 2;
        }
    })?;
    validate(Some(&root), &new, &read, &objects, &changed)?;
    let (bad, changed) = patch(&root, &objects, Family::Chapters, id(1), |r| {
        if let Record::Chapter {
            completed, page, ..
        } = &mut r[0]
        {
            *completed = true;
            *page = None;
        }
    })?;
    assert!(validate(Some(&root), &bad, &read, &objects, &changed).is_err());
    let (bad, changed) = patch(&root, &objects, Family::Chapters, id(1), |r| {
        if let Record::Chapter { completed, .. } = &mut r[0] {
            *completed = true;
        }
        if let Record::Chapter { updated_at, .. } = &mut r[1] {
            *updated_at = 2;
        }
    })?;
    assert!(
        validate(Some(&root), &bad, &read, &objects, &changed).is_err(),
        "unrelated timestamp is signed unrelated state"
    );
    let unread = Intent::read_markers(&[], &[id(1)])?;
    let (bad, changed) = patch(&root, &objects, Family::Chapters, id(1), |r| {
        r.retain(|r| r.key() != id(1))
    })?;
    assert!(
        validate(Some(&root), &bad, &unread, &objects, &changed).is_err(),
        "cannot erase a partial page"
    );
    validate(Some(&root), &root, &unread, &objects, &BTreeMap::new())?;
    let (unread_root, changed) = patch(&root, &objects, Family::Chapters, id(2), |r| {
        r.retain(|r| r.key() != id(2))
    })?;
    validate(
        Some(&root),
        &unread_root,
        &Intent::read_markers(&[], &[id(2)])?,
        &objects,
        &changed,
    )?;
    Ok(())
}
#[test]
fn status_intent_is_exact_and_null_removes_only_its_series() -> Result<()> {
    let (root, objects) = graph(
        vec![
            Record::Series {
                id: id(1),
                status: "reading".into(),
                updated_at: 1,
            },
            Record::Series {
                id: id(2),
                status: "on_hold".into(),
                updated_at: 1,
            },
            chapter(10, true, None),
        ],
        [1; 4],
    )?;
    let intent = Intent::status(id(1), Some("completed".into()))?;
    assert!(validate(Some(&root), &root, &intent, &objects, &BTreeMap::new()).is_err());
    let (new, changed) = patch(&root, &objects, Family::Series, id(1), |r| {
        if let Record::Series {
            status, updated_at, ..
        } = &mut r[0]
        {
            *status = "completed".into();
            *updated_at = 2;
        }
    })?;
    validate(Some(&root), &new, &intent, &objects, &changed)?;
    let (new, changed) = patch(&root, &objects, Family::Series, id(1), |r| {
        r.retain(|r| r.key() != id(1))
    })?;
    validate(
        Some(&root),
        &new,
        &Intent::status(id(1), None)?,
        &objects,
        &changed,
    )?;
    let (bad, changed) = patch(&root, &objects, Family::Series, id(1), |r| r.clear())?;
    assert!(
        validate(
            Some(&root),
            &bad,
            &Intent::status(id(1), None)?,
            &objects,
            &changed
        )
        .is_err()
    );
    assert!(Intent::status(id(1), Some("fictional".into())).is_err());
    Ok(())
}
#[test]
fn descriptor_rebucket_extraneous_objects_and_wrong_chunk_are_rejected() -> Result<()> {
    let (root, objects) = graph(
        vec![
            chapter(1, false, None),
            Record::List {
                id: [1; 16],
                name: "Public".into(),
                updated_at: 1,
            },
        ],
        [1; 4],
    )?;
    let intent = Intent::read_markers(&[id(1)], &[])?;
    let (new, changed) = patch(&root, &objects, Family::Chapters, id(1), |r| {
        if let Record::Chapter { completed, .. } = &mut r[0] {
            *completed = true;
        }
    })?;
    let mut extra = changed.clone();
    let list = root.families[2].chunks[0].hash;
    extra.insert(list, objects[&list].clone());
    assert!(validate(Some(&root), &new, &intent, &objects, &extra).is_err());
    let mut bad = new.clone();
    bad.families[2].chunks[0] = Descriptor::empty();
    assert!(DeltaPlan::prepare(Some(&root), &bad, &intent).is_err());
    let mut rebucket = new.clone();
    rebucket.families[0].chunks.push(Descriptor::empty());
    assert!(DeltaPlan::prepare(Some(&root), &rebucket, &intent).is_err());
    let mut corrupt = changed.clone();
    corrupt.values_mut().next().unwrap()[0] ^= 1;
    assert!(validate(Some(&root), &new, &intent, &objects, &corrupt).is_err());
    assert!(Intent::read_markers(&[], &[]).is_err());
    assert!(Intent::read_markers(&[id(1), id(1)], &[]).is_err());
    assert!(Intent::read_markers(&[id(1)], &[id(1)]).is_err());
    assert!(
        Intent::read_markers(
            &(0..=MAX_INTENT_IDS as u64).map(id).collect::<Vec<_>>(),
            &[]
        )
        .is_err()
    );
    Ok(())
}
fn large_graph(total: u64, buckets: usize) -> Result<()> {
    let target = id(total / 2);
    let (root, objects) = graph(
        (0..total).map(|n| chapter(n, false, None)).collect(),
        [buckets, 1, 1, 1],
    )?;
    assert_eq!(root.totals(&Limits::default())?.0, total);
    let (new, changed) = patch(&root, &objects, Family::Chapters, target, |r| {
        for record in r {
            if let Record::Chapter {
                id,
                completed,
                updated_at,
                ..
            } = record
            {
                if *id == target {
                    *completed = true;
                    *updated_at = 2;
                }
            }
        }
    })?;
    let plan = DeltaPlan::prepare(Some(&root), &new, &Intent::read_markers(&[target], &[])?)?;
    let mut loads = vec![];
    let stats = plan.validate(&changed, |d| {
        loads.push(d.hash);
        Ok(objects[&d.hash].clone())
    })?;
    assert_eq!(
        loads.len(),
        1,
        "unchanged unrelated chunks must never be loaded"
    );
    assert_eq!(stats.old_chunks_decoded, 1);
    assert_eq!(stats.new_chunks_decoded, 1);
    assert_eq!(stats.affected_buckets, 1);
    ensure!(
        stats.records_visited <= 8192
            && stats.records_visited < total
            && stats.bytes_decoded < 1024 * 1024,
        "bounded changed-only scan"
    );
    assert_eq!(changed.len(), 1);
    let payload =
        changed.values().map(Vec::len).sum::<usize>() + new.encode(&Limits::default())?.len();
    assert!(
        payload.div_ceil(3) * 4 + 16384 < 1024 * 1024,
        "small patch fits unchanged platform body cap, independent total graph"
    );
    Ok(())
}
#[test]
fn real_thirty_thousand_record_graph_decodes_only_affected_bucket() -> Result<()> {
    large_graph(30_000, 16)
}
#[test]
fn real_half_million_record_graph_decodes_only_affected_bucket() -> Result<()> {
    large_graph(500_000, 256)
}
#[test]
fn affected_bucket_count_is_bounded_before_any_object_load() -> Result<()> {
    let (empty, _) = graph(vec![], [128, 1, 1, 1])?;
    let mut keys = BTreeMap::new();
    for n in 0..10_000u64 {
        keys.entry(bucket_for(&id(n), 128)?).or_insert(id(n));
        if keys.len() > MAX_AFFECTED_BUCKETS {
            break;
        }
    }
    let ids = keys.into_values().collect::<Vec<_>>();
    assert_eq!(ids.len(), MAX_AFFECTED_BUCKETS + 1);
    let intent = Intent::read_markers(&ids, &[])?;
    assert!(DeltaPlan::prepare(Some(&empty), &empty, &intent).is_err());
    Ok(())
}

/// Descriptor planning bounds are checked before any loader can be invoked.
/// The large-graph tests above separately construct actual canonical chunk bytes.
#[test]
fn byte_and_record_visit_budgets_fail_during_descriptor_planning() -> Result<()> {
    for (buckets, records, size) in [(16usize, 1u64, 1_000_000u64), (64, 1024, 1000)] {
        let mut keys = BTreeMap::new();
        for n in 0..100_000u64 {
            keys.entry(bucket_for(&id(n), buckets)?).or_insert(id(n));
            if keys.len() == buckets {
                break;
            }
        }
        assert_eq!(keys.len(), buckets);
        let old = Root {
            families: Family::ALL
                .into_iter()
                .map(|family| FamilyRoot {
                    family,
                    chunks: if family == Family::Chapters {
                        (0..buckets)
                            .map(|i| Descriptor {
                                hash: id(10_000 + i as u64),
                                size,
                                records,
                            })
                            .collect()
                    } else {
                        vec![Descriptor::empty()]
                    },
                })
                .collect(),
        };
        let mut new = old.clone();
        for (i, d) in new.families[0].chunks.iter_mut().enumerate() {
            d.hash = id(20_000 + i as u64);
        }
        old.totals(&Limits::default())?;
        new.totals(&Limits::default())?;
        let intent = Intent::read_markers(&keys.into_values().collect::<Vec<_>>(), &[])?;
        let error = match DeltaPlan::prepare(Some(&old), &new, &intent) {
            Ok(_) => anyhow::bail!("scan budget unexpectedly accepted"),
            Err(e) => e,
        };
        assert!(
            error.to_string().contains("affected library scan budget"),
            "{error}"
        );
    }
    Ok(())
}
#[test]
fn changed_object_cannot_masquerade_as_another_family() -> Result<()> {
    let (old, objects) = graph(
        vec![
            chapter(1, false, None),
            Record::List {
                id: [1; 16],
                name: "Public".into(),
                updated_at: 1,
            },
        ],
        [1; 4],
    )?;
    let mut new = old.clone();
    let list = old.families[2].chunks[0].clone();
    new.families[0].chunks[0] = list.clone();
    let chunks = BTreeMap::from([(list.hash, objects[&list.hash].clone())]);
    assert!(
        validate(
            Some(&old),
            &new,
            &Intent::read_markers(&[id(1)], &[])?,
            &objects,
            &chunks
        )
        .is_err()
    );
    Ok(())
}
