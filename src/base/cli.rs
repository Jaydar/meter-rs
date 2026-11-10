use anyhow::{Context, Result, bail};

#[derive(Debug, PartialEq)]
pub enum PageEnum {
    Mac,
    Route,
    PortProxy,
    About,
}

pub struct Args {
    pub page: Option<PageEnum>,
    pub close_event: Option<String>,
}

pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Args> {
    let mut page = None;
    let mut close_event = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--page" => {
                let value = args
                    .next()
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .with_context(|| format!("missing value for {arg}"))?;

                page = Some(match value.as_str() {
                    "mac" => PageEnum::Mac,
                    "route" => PageEnum::Route,
                    "port" => PageEnum::PortProxy,
                    "about" => PageEnum::About,
                    _ => bail!("invalid page value: {}", value),
                });
            }

            "--close-event" => {
                let value = args
                    .next()
                    .filter(|v| !v.is_empty())
                    .with_context(|| format!("missing value for {arg}"))?;

                close_event = Some(value);
            }

            _ => bail!("unknown argument: {}", arg),
        }
    }

    Ok(Args { page, close_event })
}
