//! RECONSTRUCTED, UNVERIFIED bounded form query parser.
use std::collections::{BTreeSet,HashSet};
use super::{errors::{PublicApiError,PublicApiResult},PublicUuid};
pub const MAX_QUERY_BYTES:usize=32*1024;
pub const MAX_QUERY_FIELDS:usize=64;
pub const MAX_ARRAY_VALUES:usize=100;
pub const MAX_QUERY_VALUE_BYTES:usize=4096;
pub const MAX_SORT_FIELDS:usize=4;
pub const MAX_INCLUDES:usize=16;

#[derive(Clone,Debug,Default)] pub struct PublicQuery { pairs:Vec<(String,String)> }
impl PublicQuery {
    pub fn parse(raw:Option<&str>)->PublicApiResult<Self> {
        let raw=raw.unwrap_or("");
        if raw.len()>MAX_QUERY_BYTES {return Err(bad("Query is too long"));}
        if raw.is_empty(){return Ok(Self::default())}
        let mut pairs=Vec::new();
        let mut scalar=HashSet::new();
        let mut array_counts=std::collections::BTreeMap::<String,usize>::new();
        for pair in raw.split('&') {
            if pairs.len()>=MAX_QUERY_FIELDS {return Err(bad("Too many query fields"));}
            if pair.is_empty(){return Err(bad("Empty query field"));}
            let (key,value)=pair.split_once('=').unwrap_or((pair,""));
            let key=decode(key)?;let value=decode(value)?;
            if key.is_empty()||key.len()>128||value.len()>MAX_QUERY_VALUE_BYTES {
                return Err(bad("Invalid query field length"));
            }
            if key.ends_with("[]") {
                let count=array_counts.entry(key.clone()).or_default();*count+=1;
                if *count>MAX_ARRAY_VALUES{return Err(bad("Too many array values"));}
            } else if !scalar.insert(key.clone()) {return Err(bad("Duplicate query parameter"));}
            pairs.push((key,value));
        }
        Ok(Self{pairs})
    }
    pub fn first(&self,key:&str)->Option<&str> {
        self.pairs.iter().find(|(k,_)|k==key).map(|(_,v)|v.as_str())
    }
    pub fn values(&self,key:&str)->Vec<&str> {
        self.pairs.iter().filter(|(k,_)|k==key).map(|(_,v)|v.as_str()).collect()
    }
    pub fn require_allowed(&self,allowed:&[&str])->PublicApiResult<()> {
        if self.pairs.iter().any(|(key,_)|!allowed.contains(&key.as_str())) {
            return Err(bad("Unknown query parameter"));
        }
        Ok(())
    }
    pub fn bounded_usize(&self,key:&str,default:usize,min:usize,max:usize)->PublicApiResult<usize> {
        let Some(value)=self.first(key) else{return Ok(default)};
        if value.is_empty()||!value.bytes().all(|b|b.is_ascii_digit()){return Err(bad("Invalid integer query parameter"));}
        let value=value.parse::<usize>().map_err(|_|bad("Invalid integer query parameter"))?;
        if !(min..=max).contains(&value){return Err(bad("Integer query parameter out of range"));}
        Ok(value)
    }
    pub fn boolean(&self,key:&str)->PublicApiResult<Option<bool>> {
        self.first(key).map(|value|match value {
            "true"|"1"=>Ok(true),"false"|"0"=>Ok(false),_=>Err(bad("Invalid boolean query parameter")),
        }).transpose()
    }
    pub fn uuid(&self,key:&str)->PublicApiResult<Option<PublicUuid>> {
        self.first(key).map(|v|PublicUuid::parse(v).map_err(Into::into)).transpose()
    }
    pub fn uuids(&self,key:&str)->PublicApiResult<Vec<PublicUuid>> {
        self.values(key).into_iter().map(|v|PublicUuid::parse(v).map_err(Into::into)).collect()
    }
    pub fn includes(&self,allowed:&[&str])->PublicApiResult<BTreeSet<String>> {
        let values=self.values("includes[]");
        if values.len()>MAX_INCLUDES{return Err(bad("Too many relationship includes"));}
        values.into_iter().map(|value|{
            if !allowed.contains(&value){return Err(bad("Unsupported relationship include"));}
            Ok(value.to_owned())
        }).collect()
    }
    /// Preserves client sort priority. Callers map fields to fixed SQL fragments.
    pub fn order(&self,allowed:&[&str])->PublicApiResult<Vec<(String,SortDirection)>> {
        let mut order=Vec::new();
        for (key,value) in &self.pairs {
            if let Some(field)=key.strip_prefix("order[").and_then(|s|s.strip_suffix(']')) {
                if !allowed.contains(&field){return Err(bad("Unsupported order field"));}
                let direction=match value.as_str(){"asc"=>SortDirection::Asc,"desc"=>SortDirection::Desc,_=>return Err(bad("Invalid order direction"))};
                order.push((field.to_owned(),direction));
                if order.len()>MAX_SORT_FIELDS{return Err(bad("Too many order fields"));}
            }
        }
        Ok(order)
    }
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum SortDirection{Asc,Desc}
impl SortDirection {pub fn sql(self)->&'static str {match self{Self::Asc=>"ASC",Self::Desc=>"DESC"}}}
fn bad(detail:&str)->PublicApiError{PublicApiError::bad_request(detail)}
fn decode(input:&str)->PublicApiResult<String> {
    let bytes=input.as_bytes();let mut output=Vec::with_capacity(bytes.len());let mut i=0;
    while i<bytes.len(){match bytes[i]{
        b'+'=>output.push(b' '),
        b'%'=>{if i+2>=bytes.len(){return Err(bad("Invalid percent encoding"));}
            let h=hex(bytes[i+1]).ok_or_else(||bad("Invalid percent encoding"))?;
            let l=hex(bytes[i+2]).ok_or_else(||bad("Invalid percent encoding"))?;
            output.push(h*16+l);i+=2;},
        byte=>output.push(byte),
    }i+=1;}
    let output=String::from_utf8(output).map_err(|_|bad("Query must be valid UTF-8"))?;
    if output.chars().any(|c|c=='\0'||c.is_control()){return Err(bad("Control characters are not allowed"));}
    Ok(output)
}
fn hex(b:u8)->Option<u8>{match b{b'0'..=b'9'=>Some(b-b'0'),b'a'..=b'f'=>Some(b-b'a'+10),b'A'..=b'F'=>Some(b-b'A'+10),_=>None}}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn arrays_orders_and_strictness(){
        let q=PublicQuery::parse(Some("order%5BupdatedAt%5D=desc&order[title]=asc&includes[]=author&includes[]=artist")).unwrap();
        assert_eq!(q.order(&["title","updatedAt"]).unwrap()[0].0,"updatedAt");
        assert_eq!(q.includes(&["author","artist"]).unwrap().len(),2);
        for raw in ["limit=1&limit=2","x=%Q1","x=%FF","x=%00"]{assert!(PublicQuery::parse(Some(raw)).is_err());}
        assert!(PublicQuery::parse(Some("limit=-1")).unwrap().bounded_usize("limit",10,1,100).is_err());
    }
}
