"""Verify that dropping cold client-side state changes neither uploads nor roots."""
import json
from unittest.mock import patch
from run import Account,Library
out=[]
for n in (30000,500000):
 with patch('run.time.time',return_value=1700000000):
  a=Library(n,Account(77));b=Library(n,Account(77))
 before=sum(len(rows) for family in b.rows.values() for rows in family if rows)
 b.compact();after=sum(len(rows) for family in b.rows.values() for rows in family if rows)
 assert a.root==b.root
 for _ in range(5):assert a.changed()==b.changed()
 out.append(dict(completed_chapters=n,decoded_records_before=before,decoded_records_after=after,identical_five_updates=True))
print(json.dumps(out))
