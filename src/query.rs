//! The command-line options as a URL query string, for the web version: `?seed=3&no-erosion&set=
//! basal_drag%3D8e14` is `--seed=3 --no-erosion --set=basal_drag=8e14`. The page reads its
//! parameters from its URL, passes them to the simulation worker in the worker's URL, and the
//! parameter panel writes them back into the page URL on a restart, so that a setting can be
//! shared as a link.

use anyhow::{anyhow, ensure};

/// Options that are not available on the web: there are no files and no headless mode. The URL
/// parser rejects them, and the parameter panel does not show them there.
pub const WEB_UNAVAILABLE: &[&str] = &[
    "headless",
    "output",
    "width",
    "cell-dump",
    "help",
    "version",
];

/// Options that are always on in the web version: the URL accepts them, so that a desktop command
/// line carries over, but they change nothing there, and the parameter panel does not show them.
pub const WEB_IMPLIED: &[&str] = &["web-math"];

/// Converts a query string (with or without the leading `?`) into command-line arguments,
/// starting with the program name. Names may use `_` instead of `-`.
pub fn query_to_args(search: &str) -> anyhow::Result<Vec<String>> {
    let mut args = vec!["world-simulator".to_owned()];
    for pair in search
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
    {
        let (name, value) = match pair.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (pair, None),
        };
        let name = decode(name)?.replace('_', "-");
        ensure!(
            !WEB_UNAVAILABLE.contains(&name.as_str()),
            "the option `{name}` is not available on the web"
        );
        // One `--name=value` token, so that values starting with `-` are not taken as options.
        match value.filter(|value| !value.is_empty()) {
            Some(value) => args.push(format!("--{name}={}", decode(value)?)),
            None => args.push(format!("--{name}")),
        }
    }
    Ok(args)
}

/// Converts command-line arguments of the form `--name=value` or `--name` (after the program
/// name, as the parameter panel produces them) into a query string with a leading `?`, or an
/// empty string if there are none.
pub fn args_to_query(args: &[String]) -> String {
    let pairs: Vec<String> = args
        .iter()
        .skip(1)
        .map(|arg| {
            let arg = arg.trim_start_matches("--");
            match arg.split_once('=') {
                Some((name, value)) => format!("{}={}", encode(name), encode(value)),
                None => encode(arg),
            }
        })
        .collect();
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

/// Parameters of the simulation worker's URL that are not command-line options: the files of the
/// page's build that `web/worker.js` loads (the simulator's JavaScript module and its wasm file).
/// Their names carry content hashes, so that the page and its worker always run the same build
/// (issue #122).
pub const WORKER_FILE_KEYS: [&str; 2] = ["module", "wasm"];

/// The query string of the simulation worker: the files `module` and `wasm` (see
/// [`WORKER_FILE_KEYS`]), then the pairs of `query` (as [`args_to_query`] makes it).
pub fn worker_query(module: &str, wasm: &str, query: &str) -> String {
    let [module_key, wasm_key] = WORKER_FILE_KEYS;
    let mut search = format!(
        "?{module_key}={}&{wasm_key}={}",
        encode(module),
        encode(wasm)
    );
    let rest = query.trim_start_matches('?');
    if !rest.is_empty() {
        search.push('&');
        search.push_str(rest);
    }
    search
}

/// The query string `search` without the worker's file parameters (see [`WORKER_FILE_KEYS`]),
/// for [`query_to_args`].
pub fn without_worker_files(search: &str) -> String {
    let pairs: Vec<&str> = search
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| {
            let name = pair.split_once('=').map_or(*pair, |(name, _)| name);
            !pair.is_empty() && !WORKER_FILE_KEYS.contains(&name)
        })
        .collect();
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

/// Decodes a percent-encoded query component (`+` is a space).
fn decode(component: &str) -> anyhow::Result<String> {
    let invalid = || anyhow!("invalid percent-encoding in the URL: {component}");
    let mut bytes = Vec::with_capacity(component.len());
    let mut rest = component.bytes();
    while let Some(byte) = rest.next() {
        match byte {
            b'+' => bytes.push(b' '),
            b'%' => {
                let hex = [
                    rest.next().ok_or_else(invalid)?,
                    rest.next().ok_or_else(invalid)?,
                ];
                if !hex.iter().all(u8::is_ascii_hexdigit) {
                    return Err(invalid());
                }
                let hex = std::str::from_utf8(&hex).map_err(|_| invalid())?;
                bytes.push(u8::from_str_radix(hex, 16).map_err(|_| invalid())?);
            }
            byte => bytes.push(byte),
        }
    }
    String::from_utf8(bytes).map_err(|_| invalid())
}

/// Percent-encodes a query component like JavaScript's `encodeURIComponent`.
fn encode(component: &str) -> String {
    let mut encoded = String::with_capacity(component.len());
    for byte in component.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|&arg| arg.to_owned()).collect()
    }

    #[test]
    fn worker_files_go_before_the_options_and_are_removed_again() {
        let query = args_to_query(&strings(&["world-simulator", "--seed=3", "--no-erosion"]));
        let search = worker_query("./world-simulator-0a1b.js", "./x_bg.wasm", &query);
        assert_eq!(
            search,
            "?module=.%2Fworld-simulator-0a1b.js&wasm=.%2Fx_bg.wasm&seed=3&no-erosion"
        );
        assert_eq!(without_worker_files(&search), query);
        assert_eq!(
            query_to_args(&without_worker_files(&search)).unwrap(),
            strings(&["world-simulator", "--seed=3", "--no-erosion"])
        );
        let bare = worker_query("m.js", "w.wasm", "");
        assert_eq!(bare, "?module=m.js&wasm=w.wasm");
        assert_eq!(without_worker_files(&bare), "");
    }

    #[test]
    fn query_becomes_arguments() {
        let args =
            query_to_args("?seed=3&no_erosion&set=basal_drag%3D8e14&epoch=-0.5&plates=").unwrap();
        assert_eq!(
            args,
            strings(&[
                "world-simulator",
                "--seed=3",
                "--no-erosion",
                "--set=basal_drag=8e14",
                "--epoch=-0.5",
                "--plates",
            ])
        );
        assert_eq!(query_to_args("").unwrap(), strings(&["world-simulator"]));
        assert_eq!(
            query_to_args("set=a+b%C3%A9").unwrap(),
            strings(&["world-simulator", "--set=a bé"])
        );
    }

    #[test]
    fn implied_options_are_accepted() {
        use clap::Parser;
        for name in WEB_IMPLIED {
            let args = query_to_args(&format!("?{name}")).unwrap();
            assert_eq!(args, strings(&["world-simulator", &format!("--{name}")]));
            assert!(crate::Cli::try_parse_from(&args).is_ok(), "{name}");
        }
    }

    #[test]
    fn invalid_queries_are_rejected() {
        assert!(query_to_args("?output=x").is_err());
        assert!(query_to_args("?cell_dump").is_err());
        assert!(query_to_args("?seed=%4").is_err());
        assert!(query_to_args("?seed=%zz").is_err());
        assert!(query_to_args("?seed=%ff").is_err());
        assert!(query_to_args("?seed=%+f").is_err());
    }

    #[test]
    fn arguments_round_trip_through_the_query() {
        let args = strings(&[
            "world-simulator",
            "--seed=3",
            "--no-erosion",
            "--set=climate.x=1e-3",
            "--set=a b&c?é",
            "--epoch=-0.5",
        ]);
        let query = args_to_query(&args);
        assert_eq!(
            query,
            "?seed=3&no-erosion&set=climate.x%3D1e-3&set=a%20b%26c%3F%C3%A9&epoch=-0.5"
        );
        assert_eq!(query_to_args(&query).unwrap(), args);
        assert_eq!(args_to_query(&strings(&["world-simulator"])), "");
    }
}
