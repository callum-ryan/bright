use chrono::{DateTime, Local};
use clap::builder::{StringValueParser, TypedValueParser};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Pull data from Bright/GlowMarkt API")]
pub struct Cli {
    #[arg(value_parser = StringValueParser::new().try_map(parse_dt))]
    pub start_date: Option<DateTime<Local>>,
    #[arg(value_parser = StringValueParser::new().try_map(parse_dt))]
    pub end_date: Option<DateTime<Local>>,
    #[clap(env)]
    pub gm_username: String,
    #[clap(env)]
    pub gm_password: String,
    #[clap(env)]
    pub influx_uri: String,
    #[clap(env)]
    pub influx_database: String,
    #[clap(env)]
    pub influx_token: String,
    #[clap(env)]
    pub token_cache_file: Option<String>,
}

fn parse_dt(value: String) -> Result<chrono::DateTime<Local>, chrono::ParseError> {
    if let Ok(dt) = value.parse::<chrono::DateTime<Local>>() {
        Ok(dt)
    } else {
        let naive_date = value.parse::<chrono::NaiveDate>().unwrap();
        Ok(naive_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap())
    }
}
