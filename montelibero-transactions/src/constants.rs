use substrate_stellar_sdk::horizon::Horizon;

pub static MTL_FOUNDATION: &str = "GDX23CPGMQ4LN55VGEDVFZPAJMAUEHSHAMJ2GMCU2ZSHN5QF4TMZYPIS";

pub static MTL_ISSUERER: &str = "GACKTN5DAZGWXRWB2WLM6OPBDHAMT6SJNGLJZPQMEZBUR4JUGBX2UK7V";

pub static MTLCITY_ISSUERER: &str = "GDUI7JVKWZV4KJVY4EJYBXMGXC2J3ZC67Z6O5QFP4ZMVQM2U5JXK2OK3";

pub static MTL_ADDITIONAL_ACCOUNT: &str =
    "GB7NLVMVC6NWTIFK7ULLEQDF5CBCI2TDCO3OZWWSFXQCT7OPU3P4EOSR";

pub static BTC_TREASURY: &str = "GATUN5FV3QF35ZMU3C63UZ63GOFRYUHXV2SHKNTKPBZGYF2DU3B7IW6Z";

pub static BTC_FOUNDATION: &str = "GAUBJ4CTRF42Z7OM7QXTAQZG6BEMNR3JZY57Z4LB3PXSDJXE5A5GIGJB";

pub static MTL_RECT_ACCOUNT: &str = "GDASYWP6F44TVNJKZKQ2UEVZOKTENCJFTWVMP6UC7JBZGY4ZNB6YAVD4";

pub static MIN_FEE: u32 = 100;
pub static MAX_FEE: u32 = 20000;

pub static FETCH_TIMEOUT: u64 = 4000;

pub static SIGNING_TIME_WINDOW: u64 = 24 * 60 * 60;

pub fn horizon_mainnet() -> Horizon {
    Horizon::new("https://horizon.stellar.org")
}
