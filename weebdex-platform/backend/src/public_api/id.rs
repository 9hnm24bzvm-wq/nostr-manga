//! Additive public UUID syntax. Native database/storage/signed identifiers are unchanged.
use crate::error::{ApiError,ApiResult};
use serde::{Deserialize,Deserializer,Serialize,de};
use std::{fmt,str::FromStr};

#[derive(Clone,Debug,Eq,PartialEq,Ord,PartialOrd,Hash,Serialize)]
#[serde(transparent)]
pub struct PublicUuid(String);
impl PublicUuid {
    pub fn parse(value:&str)->ApiResult<Self>{
        let bytes=value.as_bytes();
        if bytes.len()!=36||bytes.iter().enumerate().any(|(i,b)|{
            if matches!(i,8|13|18|23){*b!=b'-'}else{!b.is_ascii_hexdigit()}
        }){return Err(ApiError::bad_request("Expected a public UUID"));}
        Ok(Self(value.to_ascii_lowercase()))
    }
    pub fn as_str(&self)->&str {&self.0}
    pub fn random_v4()->Self {
        let mut bytes=rand::random::<[u8;16]>();bytes[6]=(bytes[6]&0x0f)|0x40;bytes[8]=(bytes[8]&0x3f)|0x80;
        let s=hex::encode(bytes);
        Self(format!("{}-{}-{}-{}-{}",&s[..8],&s[8..12],&s[12..16],&s[16..20],&s[20..]))
    }
}
impl fmt::Display for PublicUuid {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{f.write_str(&self.0)}}
impl FromStr for PublicUuid {type Err=ApiError;fn from_str(s:&str)->Result<Self,Self::Err>{Self::parse(s)}}
impl<'de> Deserialize<'de> for PublicUuid {
    fn deserialize<D:Deserializer<'de>>(deserializer:D)->Result<Self,D::Error>{
        Self::parse(&String::deserialize(deserializer)?).map_err(|_|de::Error::custom("Expected a public UUID"))
    }
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn uuid_syntax_is_strict_and_output_canonical(){
        assert_eq!(PublicUuid::parse("11B8D076-E9A2-8F00-B100-C9981C3DF1AA").unwrap().as_str(),"11b8d076-e9a2-8f00-b100-c9981c3df1aa");
        for value in ["Manga00001","11b8d076e9a28f00b100c9981c3df1aa","11b8d076-e9a2-8f00-b100-c9981c3df1az"]{assert!(PublicUuid::parse(value).is_err());}
        assert!(PublicUuid::parse(PublicUuid::random_v4().as_str()).is_ok());
    }
}
