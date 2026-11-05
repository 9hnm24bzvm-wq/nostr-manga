//! Public, reviewed instance identity. Never serialize the runtime Config.
use crate::{AppState, ApiError, ApiResult};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use anyhow::{Result, ensure};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all="camelCase")]
pub struct InstanceInformation {
    pub operator_name: String,
    pub contact_url: Option<String>,
    pub terms_url: Option<String>,
    pub privacy_url: Option<String>,
    pub guidelines_url: Option<String>,
    pub terms: String,
    pub privacy: String,
    pub guidelines: String,
}
impl InstanceInformation {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.operator_name.len()<=200 && !self.operator_name.contains('\0'), "instance operator name exceeds its public bound");
        for (kind, link) in [("contact",&self.contact_url),("terms",&self.terms_url),("privacy",&self.privacy_url),("guidelines",&self.guidelines_url)] {
            if let Some(link)=link {
                ensure!(link.len()<=2048 && !link.chars().any(|c|c.is_control()),"invalid instance link");
                let url=url::Url::parse(link)?;
                ensure!(url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),"instance links cannot contain credentials or fragments");
                ensure!(matches!(url.scheme(),"https"|"http") && url.host_str().is_some() || kind=="contact" && url.scheme()=="mailto" && !url.path().is_empty() && url.query().is_none(),"unsupported instance link scheme");
            }
        }
        for text in [&self.terms,&self.privacy,&self.guidelines] {ensure!(text.len()<=65536 && !text.contains('\0'),"instance policy text exceeds its public bound");}
        Ok(())
    }
    /// Upload consent is meaningful only when every linked policy is supplied.
    /// Operator branding/contact are useful but are not upload-policy content.
    pub fn upload_policies_available(&self) -> bool {
        self.validate().is_ok()
            && (self.terms_url.is_some() || !self.terms.trim().is_empty())
            && (self.privacy_url.is_some() || !self.privacy.trim().is_empty())
            && (self.guidelines_url.is_some() || !self.guidelines.trim().is_empty())
    }
    pub fn require_upload_policies(&self) -> ApiResult<()> {
        if self.upload_policies_available() { Ok(()) } else {
            Err(ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "upload_policies_unavailable",
                "Chapter uploads are unavailable until the operator supplies terms, privacy policy and guidelines."))
        }
    }
    pub fn configured(&self) -> bool {
        !self.operator_name.trim().is_empty() && self.contact_url.is_some()
            && (self.terms_url.is_some()||!self.terms.trim().is_empty())
            && (self.privacy_url.is_some()||!self.privacy.trim().is_empty())
            && (self.guidelines_url.is_some()||!self.guidelines.trim().is_empty())
    }
}
pub(super) async fn information(State(s):State<AppState>)->ApiResult<Json<serde_json::Value>> {
    s.config.instance.validate().map_err(|_|ApiError::unavailable())?;
    Ok(Json(serde_json::json!({"scope":"public-instance-information","configured":s.config.instance.configured(),"uploadPoliciesAvailable":s.config.instance.upload_policies_available(),"information":s.config.instance})))
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn only_reviewable_public_links(){
        for bad in ["javascript:alert(1)","https://user:pass@example.invalid/policy","data:text/html,x","file:///private","https://example.invalid/#secret"] {
            assert!(InstanceInformation{terms_url:Some(bad.into()),..Default::default()}.validate().is_err());
        }
        assert!(InstanceInformation{contact_url:Some("mailto:help@example.invalid".into()),..Default::default()}.validate().is_ok());
        assert!(!InstanceInformation::default().configured());
    }
}

#[cfg(test)] mod upload_policy_tests {
    use super::*;
    #[test] fn requires_all_three_reviewable_policies_but_not_branding() {
        let complete = InstanceInformation { terms:"Terms".into(), privacy:"Privacy".into(), guidelines:"Guidelines".into(), ..Default::default() };
        assert!(complete.upload_policies_available());
        assert!(!complete.configured());
        for missing in 0..3 {
            let mut value=complete.clone();
            match missing { 0=>value.terms=" \n".into(), 1=>value.privacy.clear(), _=>value.guidelines.clear() }
            assert!(!value.upload_policies_available());
            let error=value.require_upload_policies().unwrap_err();
            assert_eq!(error.status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(error.code, "upload_policies_unavailable");
        }
    }
    #[test] fn reviewed_links_are_valid_alternatives_not_unchecked_presence() {
        let mut value=InstanceInformation { terms_url:Some("https://example.invalid/terms".into()), privacy_url:Some("https://example.invalid/privacy".into()), guidelines_url:Some("https://example.invalid/guidelines".into()), ..Default::default() };
        assert!(value.upload_policies_available());
        for bad in ["", "javascript:alert(1)", "https://user:pass@example.invalid/", "https://example.invalid/#secret"] {
            value.terms_url=Some(bad.into()); assert!(!value.upload_policies_available());
        }
    }
}
