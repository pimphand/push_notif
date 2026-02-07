use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use std::io::BufReader;
use std::path::Path;
use tracing::{error, info};
use web_push::{
    ContentEncoding, IsahcWebPushClient, PartialVapidSignatureBuilder, SubscriptionInfo,
    VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder,
};

const VAPID_PRIVATE_PEM: &str = "private.pem";

pub struct PushService {
    vapid_builder: PartialVapidSignatureBuilder,
    client: IsahcWebPushClient,
}

impl PushService {
    pub fn new() -> anyhow::Result<Self> {
        let path = Path::new(VAPID_PRIVATE_PEM);
        if !path.exists() {
            anyhow::bail!(
                "VAPID private key not found at {}. Generate with: openssl ecparam -name prime256v1 -genkey -noout -out private.pem",
                VAPID_PRIVATE_PEM
            );
        }
        let file = std::fs::File::open(path)?;
        let vapid_builder = VapidSignatureBuilder::from_pem_no_sub(BufReader::new(file))?;
        let client = IsahcWebPushClient::new()?;
        Ok(Self {
            vapid_builder,
            client,
        })
    }

    pub fn public_key_base64url(&self) -> String {
        let bytes = self.vapid_builder.get_public_key();
        URL_SAFE_NO_PAD.encode(bytes)
    }

    pub async fn send(&self, subscription: &SubscriptionInfo, payload: &[u8]) -> Result<(), web_push::WebPushError> {
        let sig_builder = self.vapid_builder.clone();
        let vapid_sig = sig_builder
            .add_sub_info(subscription)
            .build()?;

        let mut builder = WebPushMessageBuilder::new(subscription);
        builder.set_payload(ContentEncoding::Aes128Gcm, payload);
        builder.set_vapid_signature(vapid_sig);

        self.client.send(builder.build()?).await
    }
}

pub async fn send_to_all(
    push_service: &PushService,
    subscriptions: &[SubscriptionInfo],
    payload: &[u8],
) -> (usize, usize) {
    let mut ok = 0;
    let mut fail = 0;
    for sub in subscriptions {
        match push_service.send(sub, payload).await {
            Ok(()) => {
                ok += 1;
                info!(endpoint = %sub.endpoint, "push sent");
            }
            Err(e) => {
                fail += 1;
                error!(endpoint = %sub.endpoint, error = %e, "push failed");
            }
        }
    }
    (ok, fail)
}
