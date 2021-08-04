use substrate_stellar_sdk::{
    horizon::Horizon
};

pub static MTL_FOUNDATION: &str = "GDX23CPGMQ4LN55VGEDVFZPAJMAUEHSHAMJ2GMCU2ZSHN5QF4TMZYPIS";

pub static MTL_ISSUERER: &str = "GACKTN5DAZGWXRWB2WLM6OPBDHAMT6SJNGLJZPQMEZBUR4JUGBX2UK7V";

pub static MTLCITY_ISSUERER: &str = "GDUI7JVKWZV4KJVY4EJYBXMGXC2J3ZC67Z6O5QFP4ZMVQM2U5JXK2OK3";

pub static MIN_FEE: u32 = 100;
pub static MAX_FEE: u32 = 1000;

pub static FETCH_TIMEOUT: u64 = 4000;

pub static SIGNING_TIME_WINDOW: u64 = 24 * 60 * 60;

pub fn horizon_mainnet() -> Horizon {
    Horizon::new("https://horizon.stellar.org")
}