use std::env;
use std::error::Error;
use std::sync::Arc;

use reqwest::{ClientBuilder};
use scraper::{Html, Selector};
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::{Duration};
use tokio::time::interval;
use url::Url;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: linkz <url> <interval>");
        return;
    }
    let url = &args[1];
    let url_to_check = Arc::new(Mutex::new(url.clone()));
    let interval_from_arg = &args[2].parse::<u64>().unwrap();

    let mut interval = interval(Duration::from_secs(*interval_from_arg));
    let shared_links = Arc::new(Mutex::new(Vec::<String>::new()));
    loop {
        interval.tick().await;

        let old_links = Arc::clone(&shared_links);
        let url_arc = Arc::clone(&url_to_check);

        tokio::spawn(async move {
            let mut links_data = old_links.lock().await;
            let url = url_arc.lock().await;

            let result = fetch_links(&url).await;
            match result {
                Ok(links) => {
                    let new_links = diff(&links_data, links.clone());
                    if new_links.is_empty() {
                        return;
                    }
                    for link in new_links {
                        links_data.push(link.clone());
                        println!("{}", link);
                    }
                }
                Err(err) => eprintln!("Error: {}", err)
            }
        });
    }
}


fn diff(old: &MutexGuard<Vec<String>>, new: Vec<String>) -> Vec<String> {
    let mut diff: Vec<String> = Vec::new();
    for link in new {
        if !old.contains(&link) {
            diff.push(link);
        }
    }
    diff
}

fn validate_url(url: &str) -> bool {
    let url_parsed = Url::parse(url);
    match url_parsed {
        Ok(u) => {
            // Check if the URL has a scheme (e.g., http, https)
            if u.scheme().is_empty() {
                return false;
            }

            // Check if the URL has a host
            if u.host().is_none() {
                return false;
            }
            true
        }
        Err(_) => {
            false
        }
    }
}


fn create_absolute_url(base_url: &str, relative_link: &str) -> Result<String, url::ParseError> {
    let base = Url::parse(base_url)?;
    let absolute_url = base.join(relative_link)?;
    Ok(absolute_url.to_string())
}

fn default_exclusions() -> Vec<String> {
    let mut exclusions = Vec::new();
    exclusions.push("javascript:void(0)");
    exclusions.push("mailto:");
    return exclusions.iter().map(|s| s.to_string()).collect();
}

async fn fetch_links(url: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let client = ClientBuilder::new().danger_accept_invalid_certs(true).build().expect("Failed to build client");
    let res = client.get(url).send().await?;
    let body = res.text().await?;

    let fragment = Html::parse_document(&body);
    let selector = Selector::parse("a").unwrap();

    let mut links: Vec<String> = fragment
        .select(&selector)
        .filter_map(|link| link.value().attr("href"))
        .map(|href| {
            let link = href.to_string();
            if validate_url(&link) {
                link
            } else {
                create_absolute_url(url, &link).unwrap()
            }
        })
        .filter(|link| !default_exclusions().iter().any(|ex| link.contains(ex)))
        .collect();

    links.sort();
    links.dedup();

    Ok(links)
}
