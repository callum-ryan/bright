## bright

Pull energy usage data from the GlowMarkt/Bright API, and push to InfluxDB.

## installation
```bash
$ git clone git@github.com:callum-ryan/bright.git
$ cd bright
$ cargo build --release && cp target/release/bright .
```

## usage
the command line arguments can be set as environment variables too - to avoid some degree of credential leakage.

```bash
$ ./bright --help
Pull data from Bright/GlowMarkt API

Usage: bright [START_DATE] [END_DATE] <GM_USERNAME> <GM_PASSWORD> <INFLUX_URI> <INFLUX_DATABASE> <INFLUX_TOKEN> [TOKEN_CACHE_FILE]

Arguments:
  [START_DATE]
  [END_DATE]
  <GM_USERNAME>       [env: GM_USERNAME=]
  <GM_PASSWORD>       [env: GM_PASSWORD=]
  <INFLUX_URI>        [env: INFLUX_URI=]
  <INFLUX_DATABASE>   [env: INFLUX_DATABASE=]
  <INFLUX_TOKEN>      [env: INFLUX_TOKEN=]
  [TOKEN_CACHE_FILE]  [env: TOKEN_CACHE_FILE=]
```

`TOKEN_CACHE_FILE`, when set, can cache the token that the application will use to call the API as these are relatively long-lived.

if all vars are set via environment variables, then `START_DATE` and `END_DATE` are optional, and it will pull the last 10 days of usage.
