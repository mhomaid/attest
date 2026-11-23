//! Synthetic commerce profile for the load generator.
//!
//! Normal storefront traffic is drawn from [`CommerceConfig`]: session length,
//! view-to-cart ratio, and login frequency. Each attack is a switch with a
//! ground-truth label. A seed fixes ids and timestamps, so the same config
//! yields the same [`output_hash`].
//!
//! Patterns only. No real card numbers, customers, or merchants.

use std::collections::VecDeque;

use attest_common::FlatEvent;
use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const EPOCH: i64 = 1_767_225_600; // 2026-01-01T00:00:00Z
const ACCOUNT: &str = "123456789012";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommerceLabel {
    Normal,
    SessionReuseAfterReset,
    OtpBurst,
    CardTesting,
    InjectedStorefrontScript,
    CatalogueScraping,
    EdgeRuleBypass,
    BlockedPaymentCallback,
    /// Original single-event edge block helper.
    #[cfg(test)]
    EdgeBlock,
}

#[derive(Debug)]
pub struct CommerceEvent {
    pub event: FlatEvent,
    pub label: CommerceLabel,
}

/// Which labelled attacks `generate` and [`CommerceStream`] append.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attacks {
    pub session_reuse_after_reset: bool,
    pub otp_burst: bool,
    pub card_testing: bool,
    pub injected_storefront_script: bool,
    pub catalogue_scraping: bool,
    pub edge_rule_bypass: bool,
    pub blocked_payment_callback: bool,
}

impl Attacks {
    #[cfg(test)]
    pub fn none() -> Self {
        Self {
            session_reuse_after_reset: false,
            otp_burst: false,
            card_testing: false,
            injected_storefront_script: false,
            catalogue_scraping: false,
            edge_rule_bypass: false,
            blocked_payment_callback: false,
        }
    }

    pub fn all() -> Self {
        Self {
            session_reuse_after_reset: true,
            otp_burst: true,
            card_testing: true,
            injected_storefront_script: true,
            catalogue_scraping: true,
            edge_rule_bypass: true,
            blocked_payment_callback: true,
        }
    }
}

impl Default for Attacks {
    fn default() -> Self {
        Self::all()
    }
}

/// Distribution parameters for normal storefront behaviour, plus attack switches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommerceConfig {
    /// Centre of the per-session event count.
    pub session_length: u32,
    /// Probability that a product view is followed by add-to-cart, in `0.0..=1.0`.
    pub view_to_cart_ratio: f64,
    /// Probability that a session starts with a login, in `0.0..=1.0`.
    pub login_frequency: f64,
    pub seed: u64,
    pub tenant_id: String,
    pub store_id: String,
    pub src_ip: String,
    /// Normal events emitted before the enabled attack sequences.
    pub normal_events: usize,
    pub attacks: Attacks,
}

impl Default for CommerceConfig {
    fn default() -> Self {
        Self {
            session_length: 6,
            view_to_cart_ratio: 0.25,
            login_frequency: 0.4,
            seed: 53,
            tenant_id: "tenant-a".into(),
            store_id: "1001".into(),
            src_ip: "203.0.113.10".into(),
            normal_events: 24,
            attacks: Attacks::all(),
        }
    }
}

#[derive(Clone, Copy)]
enum AttackKind {
    SessionReuseAfterReset,
    OtpBurst,
    CardTesting,
    InjectedStorefrontScript,
    CatalogueScraping,
    EdgeRuleBypass,
    BlockedPaymentCallback,
}

fn enabled_attacks(attacks: &Attacks) -> Vec<AttackKind> {
    let mut kinds = Vec::new();
    if attacks.session_reuse_after_reset {
        kinds.push(AttackKind::SessionReuseAfterReset);
    }
    if attacks.otp_burst {
        kinds.push(AttackKind::OtpBurst);
    }
    if attacks.card_testing {
        kinds.push(AttackKind::CardTesting);
    }
    if attacks.injected_storefront_script {
        kinds.push(AttackKind::InjectedStorefrontScript);
    }
    if attacks.catalogue_scraping {
        kinds.push(AttackKind::CatalogueScraping);
    }
    if attacks.edge_rule_bypass {
        kinds.push(AttackKind::EdgeRuleBypass);
    }
    if attacks.blocked_payment_callback {
        kinds.push(AttackKind::BlockedPaymentCallback);
    }
    kinds
}

/// One finite, labelled trace. Same config and seed → same events.
#[cfg(test)]
pub fn generate(cfg: &CommerceConfig) -> Vec<CommerceEvent> {
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let mut seq = 0u64;
    let mut session_n = 0u64;
    let mut out = Vec::new();
    let target = cfg.normal_events.max(1);
    while out.len() < target {
        out.extend(normal_session(&mut rng, cfg, &mut seq, &mut session_n));
    }
    for kind in enabled_attacks(&cfg.attacks) {
        out.extend(emit_attack(kind, &mut rng, cfg, &mut seq));
    }
    out
}

/// FNV-1a 64 of the label and canonical JSON for each event.
#[cfg(test)]
pub fn output_hash(events: &[CommerceEvent]) -> String {
    let mut hasher = Fnv64::new();
    for ev in events {
        hasher.write(label_name(ev.label).as_bytes());
        hasher.write(b"\n");
        let payload = serde_json::to_string(&ev.event).unwrap_or_default();
        hasher.write(payload.as_bytes());
        hasher.write(b"\n");
    }
    format!("{:016x}", hasher.finish())
}

/// Endless labelled stream used by the `commerce` scenario.
pub struct CommerceStream {
    cfg: CommerceConfig,
    rng: StdRng,
    seq: u64,
    session_n: u64,
    pending: VecDeque<CommerceEvent>,
    attack_cursor: usize,
}

impl CommerceStream {
    pub fn new(cfg: CommerceConfig) -> Self {
        let rng = StdRng::seed_from_u64(cfg.seed);
        Self {
            cfg,
            rng,
            seq: 0,
            session_n: 0,
            pending: VecDeque::new(),
            attack_cursor: 0,
        }
    }

    pub fn next_event(&mut self) -> CommerceEvent {
        if self.pending.is_empty() {
            self.refill();
        }
        self.pending
            .pop_front()
            .expect("commerce refill emits a session")
    }

    fn refill(&mut self) {
        let session = normal_session(&mut self.rng, &self.cfg, &mut self.seq, &mut self.session_n);
        self.pending.extend(session);
        let kinds = enabled_attacks(&self.cfg.attacks);
        if kinds.is_empty() {
            return;
        }
        let kind = kinds[self.attack_cursor % kinds.len()];
        self.attack_cursor = self.attack_cursor.wrapping_add(1);
        let attack = emit_attack(kind, &mut self.rng, &self.cfg, &mut self.seq);
        self.pending.extend(attack);
    }
}

fn emit_attack(
    kind: AttackKind,
    rng: &mut StdRng,
    cfg: &CommerceConfig,
    seq: &mut u64,
) -> Vec<CommerceEvent> {
    match kind {
        AttackKind::SessionReuseAfterReset => session_reuse(rng, cfg, seq),
        AttackKind::OtpBurst => otp_burst_seq(rng, cfg, seq),
        AttackKind::CardTesting => card_testing_seq(rng, cfg, seq),
        AttackKind::InjectedStorefrontScript => injected_script(rng, cfg, seq),
        AttackKind::CatalogueScraping => catalogue_scrape(rng, cfg, seq),
        AttackKind::EdgeRuleBypass => edge_bypass(rng, cfg, seq),
        AttackKind::BlockedPaymentCallback => blocked_callback(rng, cfg, seq),
    }
}

fn normal_session(
    rng: &mut StdRng,
    cfg: &CommerceConfig,
    seq: &mut u64,
    session_n: &mut u64,
) -> Vec<CommerceEvent> {
    let len = draw_session_len(rng, cfg.session_length);
    let session_uid = format!("session-{session_n}");
    *session_n += 1;
    let store = cfg.store_id.clone();
    let (body_len, with_checkout) = if len >= 4 {
        (len - 2, true)
    } else {
        (len, false)
    };
    let mut events = Vec::with_capacity(len);
    if body_len > 0 && rng.gen_bool(probability(cfg.login_frequency)) {
        events.push(make(
            rng,
            cfg,
            seq,
            "3002",
            CommerceLabel::Normal,
            |event| {
                tag_session(event, &session_uid);
                event.auth_status = Some("Success".into());
                event.api_operation = Some("login".into());
                event.http_path = Some(format!("/stores/{store}/login"));
                event.http_status = Some("200".into());
            },
        ));
    }
    let ratio = probability(cfg.view_to_cart_ratio);
    while events.len() < body_len {
        let product = rng.gen_range(1u32..40);
        events.push(make(
            rng,
            cfg,
            seq,
            "4002",
            CommerceLabel::Normal,
            |event| {
                tag_session(event, &session_uid);
                event.api_operation = Some("product_view".into());
                event.http_path = Some(format!("/stores/{store}/products/{product}"));
                event.http_status = Some("200".into());
            },
        ));
        if events.len() < body_len && rng.gen_bool(ratio) {
            events.push(make(
                rng,
                cfg,
                seq,
                "4002",
                CommerceLabel::Normal,
                |event| {
                    tag_session(event, &session_uid);
                    event.api_operation = Some("add_to_cart".into());
                    event.http_path = Some(format!("/stores/{store}/cart"));
                    event.http_status = Some("200".into());
                },
            ));
        }
    }
    if with_checkout {
        events.push(make(
            rng,
            cfg,
            seq,
            "4002",
            CommerceLabel::Normal,
            |event| {
                tag_session(event, &session_uid);
                event.api_operation = Some("checkout".into());
                event.http_path = Some(format!("/stores/{store}/checkout"));
                event.http_status = Some("200".into());
            },
        ));
        events.push(make(
            rng,
            cfg,
            seq,
            "4002",
            CommerceLabel::Normal,
            |event| {
                tag_session(event, &session_uid);
                event.api_operation = Some("payment_callback".into());
                event.http_path = Some(format!("/stores/{store}/payments/callback"));
                event.http_status = Some("200".into());
                event.disposition = Some("Allowed".into());
            },
        ));
    }
    events
}

fn draw_session_len(rng: &mut StdRng, mean: u32) -> usize {
    let mean = mean.max(1);
    let low = (mean / 2).max(1);
    let high = (mean.saturating_mul(3) / 2).max(low);
    let mut len = rng.gen_range(low..=high);
    if mean >= 4 {
        len = len.max(4);
    }
    len as usize
}

fn session_reuse(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    let mut out = Vec::with_capacity(2);
    out.push(make(
        rng,
        cfg,
        seq,
        "3002",
        CommerceLabel::SessionReuseAfterReset,
        |event| {
            event.auth_status = Some("Success".into());
            event.api_operation = Some("password_reset".into());
            event.http_path = Some(format!("/stores/{store}/password-reset"));
            event.http_status = Some("200".into());
            event.severity = "medium".into();
        },
    ));
    out.push(make(
        rng,
        cfg,
        seq,
        "3002",
        CommerceLabel::SessionReuseAfterReset,
        |event| {
            event.auth_status = Some("Success".into());
            event.api_operation = Some("session_reuse".into());
            event.http_path = Some(format!("/stores/{store}/account"));
            event.http_status = Some("200".into());
            event.severity = "high".into();
        },
    ));
    out
}

fn otp_burst_seq(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    (0..6)
        .map(|_| {
            make(rng, cfg, seq, "3002", CommerceLabel::OtpBurst, |event| {
                event.auth_status = Some("Failure".into());
                event.api_operation = Some("otp_request".into());
                event.http_path = Some(format!("/stores/{store}/otp"));
                event.severity = "high".into();
            })
        })
        .collect()
}

fn card_testing_seq(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    // Repeated checkout declines. No PAN or payment token is stored.
    (0..5)
        .map(|_| {
            make(rng, cfg, seq, "4002", CommerceLabel::CardTesting, |event| {
                event.api_operation = Some("card_testing".into());
                event.http_path = Some(format!("/stores/{store}/checkout"));
                event.http_status = Some("402".into());
                event.severity = "high".into();
            })
        })
        .collect()
}

fn injected_script(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    vec![make(
        rng,
        cfg,
        seq,
        "4002",
        CommerceLabel::InjectedStorefrontScript,
        |event| {
            event.api_operation = Some("injected_storefront_script".into());
            event.http_path = Some(format!("/stores/{store}/pages/home"));
            event.http_status = Some("200".into());
            event.severity = "high".into();
        },
    )]
}

fn catalogue_scrape(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    (0..8)
        .map(|i| {
            make(
                rng,
                cfg,
                seq,
                "4002",
                CommerceLabel::CatalogueScraping,
                |event| {
                    event.api_operation = Some("catalogue_scrape".into());
                    event.http_path = Some(format!("/stores/{store}/catalogue/{i}"));
                    event.http_status = Some("200".into());
                    event.severity = "medium".into();
                },
            )
        })
        .collect()
}

fn edge_bypass(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    (0..4)
        .map(|_| {
            make(
                rng,
                cfg,
                seq,
                "4002",
                CommerceLabel::EdgeRuleBypass,
                |event| {
                    event.api_operation = Some("edge_rule_bypass".into());
                    event.http_path = Some(format!("/stores/{store}/edge-probe"));
                    event.http_status = Some("403".into());
                    event.disposition = Some("Blocked".into());
                    event.severity = "high".into();
                },
            )
        })
        .collect()
}

fn blocked_callback(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64) -> Vec<CommerceEvent> {
    let store = cfg.store_id.clone();
    vec![make(
        rng,
        cfg,
        seq,
        "4002",
        CommerceLabel::BlockedPaymentCallback,
        |event| {
            event.api_operation = Some("payment_callback".into());
            event.http_path = Some(format!("/stores/{store}/payments/callback"));
            event.http_status = Some("403".into());
            event.disposition = Some("Blocked".into());
            event.severity = "high".into();
        },
    )]
}

fn tag_session(event: &mut FlatEvent, session_uid: &str) {
    event.actor_user_uid = session_uid.to_string();
    event.actor_user_name = "shopper-1001".into();
}

fn make(
    rng: &mut StdRng,
    cfg: &CommerceConfig,
    seq: &mut u64,
    class_uid: &str,
    label: CommerceLabel,
    fill: impl FnOnce(&mut FlatEvent),
) -> CommerceEvent {
    let mut event = skeleton(rng, cfg, seq, class_uid);
    fill(&mut event);
    CommerceEvent { event, label }
}

fn skeleton(rng: &mut StdRng, cfg: &CommerceConfig, seq: &mut u64, class_uid: &str) -> FlatEvent {
    FlatEvent {
        event_id: event_id(rng),
        class_uid: class_uid.to_string(),
        time: stamp(seq),
        tenant_id: cfg.tenant_id.clone(),
        actor_user_name: "shopper-1001".into(),
        actor_user_uid: "user-1001".into(),
        cloud_region: "us-east-1".into(),
        cloud_account_uid: ACCOUNT.into(),
        severity: "informational".into(),
        auth_status: None,
        api_operation: None,
        api_service: Some("storefront".into()),
        raw: None,
        http_path: None,
        http_status: None,
        src_ip: Some(cfg.src_ip.clone()),
        asn: Some("AS64500".into()),
        disposition: None,
    }
}

fn event_id(rng: &mut StdRng) -> String {
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes).to_string()
}

fn stamp(seq: &mut u64) -> String {
    let secs = EPOCH.saturating_add(i64::try_from(*seq).unwrap_or(i64::MAX));
    *seq = seq.saturating_add(1);
    DateTime::<Utc>::from_timestamp(secs, 0)
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| "2026-01-01T00:00:00+00:00".to_string())
}

fn probability(p: f64) -> f64 {
    if p.is_finite() {
        p.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
fn label_name(label: CommerceLabel) -> &'static str {
    match label {
        CommerceLabel::Normal => "normal",
        CommerceLabel::SessionReuseAfterReset => "session_reuse_after_reset",
        CommerceLabel::OtpBurst => "otp_burst",
        CommerceLabel::CardTesting => "card_testing",
        CommerceLabel::InjectedStorefrontScript => "injected_storefront_script",
        CommerceLabel::CatalogueScraping => "catalogue_scraping",
        CommerceLabel::EdgeRuleBypass => "edge_rule_bypass",
        CommerceLabel::BlockedPaymentCallback => "blocked_payment_callback",
        CommerceLabel::EdgeBlock => "edge_block",
    }
}

#[cfg(test)]
struct Fnv64(u64);

#[cfg(test)]
impl Fnv64 {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }

    fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
fn live_base(class_uid: &str, tenant: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: class_uid.into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant.into(),
        actor_user_name: "shopper-1001".into(),
        actor_user_uid: "user-1001".into(),
        cloud_region: "us-east-1".into(),
        cloud_account_uid: ACCOUNT.into(),
        severity: "medium".into(),
        auth_status: None,
        api_operation: None,
        api_service: None,
        raw: None,
        http_path: None,
        http_status: None,
        src_ip: Some("203.0.113.10".into()),
        asn: Some("AS64500".into()),
        disposition: None,
    }
}

#[cfg(test)]
pub fn card_testing(tenant: &str) -> CommerceEvent {
    let mut event = live_base("4002", tenant);
    event.http_path = Some("/checkout".into());
    event.http_status = Some("402".into());
    event.api_operation = Some("card_testing".into());
    event.severity = "high".into();
    CommerceEvent {
        event,
        label: CommerceLabel::CardTesting,
    }
}

#[cfg(test)]
pub fn otp_burst(tenant: &str) -> CommerceEvent {
    let mut event = live_base("3002", tenant);
    event.auth_status = Some("Failure".into());
    event.api_operation = Some("otp_request".into());
    CommerceEvent {
        event,
        label: CommerceLabel::OtpBurst,
    }
}

#[cfg(test)]
pub fn edge_block(tenant: &str, path: &str) -> CommerceEvent {
    let mut event = live_base("4002", tenant);
    event.http_path = Some(path.into());
    event.http_status = Some("403".into());
    event.disposition = Some("Blocked".into());
    CommerceEvent {
        event,
        label: CommerceLabel::EdgeBlock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(ev: &CommerceEvent) -> &str {
        ev.event.api_operation.as_deref().unwrap_or("")
    }

    fn normal_sessions(events: &[CommerceEvent]) -> Vec<Vec<&CommerceEvent>> {
        let mut sessions: Vec<Vec<&CommerceEvent>> = Vec::new();
        for ev in events {
            if ev.label != CommerceLabel::Normal {
                continue;
            }
            let same = sessions.last().is_some_and(|group: &Vec<_>| {
                group
                    .first()
                    .is_some_and(|first| first.event.actor_user_uid == ev.event.actor_user_uid)
            });
            if same {
                sessions.last_mut().expect("group").push(ev);
            } else {
                sessions.push(vec![ev]);
            }
        }
        sessions
    }

    fn has_long_digit_run(value: &str) -> bool {
        let mut run = 0;
        for ch in value.chars() {
            if ch.is_ascii_digit() {
                run += 1;
                if run >= 13 {
                    return true;
                }
            } else {
                run = 0;
            }
        }
        false
    }

    #[test]
    fn pack_covers_card_testing_otp_and_edge_block() {
        let card = card_testing("tenant-a");
        assert_eq!(card.label, CommerceLabel::CardTesting);
        assert_eq!(card.event.class_uid, "4002");
        assert_eq!(card.event.http_path.as_deref(), Some("/checkout"));
        assert!(card.event.raw.is_none());

        let otp = otp_burst("tenant-a");
        assert_eq!(otp.label, CommerceLabel::OtpBurst);
        assert_eq!(otp.event.class_uid, "3002");
        assert_eq!(otp.event.auth_status.as_deref(), Some("Failure"));

        let edge = edge_block("tenant-a", "/hooks/callback");
        assert_eq!(edge.label, CommerceLabel::EdgeBlock);
        assert_eq!(edge.event.disposition.as_deref(), Some("Blocked"));
        assert_eq!(edge.event.http_path.as_deref(), Some("/hooks/callback"));
    }

    #[test]
    fn same_seed_same_output_hash() {
        let cfg = CommerceConfig {
            seed: 42,
            normal_events: 24,
            attacks: Attacks::all(),
            ..CommerceConfig::default()
        };
        let first = generate(&cfg);
        let second = generate(&cfg);
        let hash = output_hash(&first);
        assert_eq!(hash, output_hash(&second));
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn different_seed_different_output_hash() {
        let base = CommerceConfig {
            seed: 1,
            normal_events: 16,
            attacks: Attacks::all(),
            ..CommerceConfig::default()
        };
        let other = CommerceConfig {
            seed: 2,
            ..base.clone()
        };
        assert_ne!(
            output_hash(&generate(&base)),
            output_hash(&generate(&other))
        );
    }

    #[test]
    fn session_length_view_to_cart_and_login_follow_config() {
        let mut quiet = CommerceConfig {
            seed: 11,
            session_length: 8,
            normal_events: 48,
            attacks: Attacks::none(),
            login_frequency: 0.0,
            view_to_cart_ratio: 0.0,
            ..CommerceConfig::default()
        };
        let no_cart = generate(&quiet);
        let sessions = normal_sessions(&no_cart);
        assert!(!sessions.is_empty());
        for session in &sessions {
            assert!((4..=12).contains(&session.len()));
            assert!(session.iter().all(|ev| op(ev) != "login"));
            assert!(session.iter().all(|ev| op(ev) != "add_to_cart"));
        }
        assert!(no_cart.iter().any(|ev| op(ev) == "product_view"));

        quiet.view_to_cart_ratio = 1.0;
        let with_cart = generate(&quiet);
        let sessions = normal_sessions(&with_cart);
        let views = with_cart
            .iter()
            .filter(|ev| op(ev) == "product_view")
            .count();
        let carts = with_cart
            .iter()
            .filter(|ev| op(ev) == "add_to_cart")
            .count();
        assert!(carts > 0);
        assert!(views <= carts + sessions.len());

        quiet.view_to_cart_ratio = 0.0;
        quiet.login_frequency = 1.0;
        let with_login = generate(&quiet);
        let sessions = normal_sessions(&with_login);
        assert!(sessions.iter().all(|session| op(session[0]) == "login"));
    }

    #[test]
    fn attacks_are_switchable_and_labelled() {
        let mut cfg = CommerceConfig {
            seed: 3,
            normal_events: 4,
            attacks: Attacks::none(),
            ..CommerceConfig::default()
        };
        let quiet = generate(&cfg);
        assert!(quiet.iter().all(|ev| ev.label == CommerceLabel::Normal));

        cfg.attacks.otp_burst = true;
        let otp_only = generate(&cfg);
        assert!(otp_only
            .iter()
            .any(|ev| ev.label == CommerceLabel::OtpBurst));
        assert!(otp_only
            .iter()
            .filter(|ev| ev.label == CommerceLabel::OtpBurst)
            .all(
                |ev| op(ev) == "otp_request" && ev.event.auth_status.as_deref() == Some("Failure")
            ));
        assert!(otp_only
            .iter()
            .all(|ev| ev.label != CommerceLabel::CardTesting));
        assert_ne!(output_hash(&quiet), output_hash(&otp_only));

        cfg.attacks = Attacks::all();
        let all = generate(&cfg);
        let expect = [
            (CommerceLabel::SessionReuseAfterReset, "session_reuse"),
            (CommerceLabel::OtpBurst, "otp_request"),
            (CommerceLabel::CardTesting, "card_testing"),
            (
                CommerceLabel::InjectedStorefrontScript,
                "injected_storefront_script",
            ),
            (CommerceLabel::CatalogueScraping, "catalogue_scrape"),
            (CommerceLabel::EdgeRuleBypass, "edge_rule_bypass"),
            (CommerceLabel::BlockedPaymentCallback, "payment_callback"),
        ];
        for (label, operation) in expect {
            assert!(
                all.iter()
                    .any(|ev| ev.label == label && op(ev) == operation),
                "missing {label:?}"
            );
        }
        let reuse: Vec<_> = all
            .iter()
            .filter(|ev| ev.label == CommerceLabel::SessionReuseAfterReset)
            .map(op)
            .collect();
        assert_eq!(reuse, ["password_reset", "session_reuse"]);
        assert!(all
            .iter()
            .filter(|ev| ev.label == CommerceLabel::CardTesting)
            .all(|ev| {
                ev.event.raw.is_none()
                    && ev.event.http_status.as_deref() == Some("402")
                    && !has_long_digit_run(ev.event.http_path.as_deref().unwrap_or(""))
                    && !has_long_digit_run(op(ev))
                    && !has_long_digit_run(&ev.event.actor_user_name)
            }));
    }

    #[test]
    fn defaults_use_tenant_store_and_test_net_ip() {
        let events = generate(&CommerceConfig::default());
        assert!(events.iter().any(|ev| op(ev) == "checkout"));
        assert!(events.iter().any(|ev| {
            ev.label == CommerceLabel::Normal
                && op(ev) == "payment_callback"
                && ev.event.disposition.as_deref() == Some("Allowed")
        }));
        assert!(events.iter().all(|ev| {
            ev.event.tenant_id == "tenant-a"
                && ev.event.src_ip.as_deref() == Some("203.0.113.10")
                && ev
                    .event
                    .http_path
                    .as_deref()
                    .unwrap_or("")
                    .contains("/stores/1001/")
        }));
    }

    #[test]
    fn stream_matches_its_seed() {
        let cfg = CommerceConfig {
            seed: 9,
            attacks: Attacks::all(),
            ..CommerceConfig::default()
        };
        let mut left = CommerceStream::new(cfg.clone());
        let mut right = CommerceStream::new(cfg);
        let a: Vec<_> = (0..40).map(|_| left.next_event()).collect();
        let b: Vec<_> = (0..40).map(|_| right.next_event()).collect();
        assert_eq!(output_hash(&a), output_hash(&b));
        assert!(a.iter().any(|ev| ev.label == CommerceLabel::Normal));
        assert!(a.iter().any(|ev| ev.label != CommerceLabel::Normal));
        assert_eq!(a[0].event.src_ip.as_deref(), Some("203.0.113.10"));
    }
}
