use chrono::{FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::Duration;

const GAP_THRESHOLD_PCT: f64 = 2.0;

#[derive(Debug, Deserialize)]
struct StockSnapshot {
    symbol: String,
    prev_close: f64,
    pre_open_price: f64,
}

#[derive(Serialize)]
struct TelegramPayload<'a> {
    chat_id: &'a str,
    text: &'a str,
    parse_mode: &'a str,
}

fn calculate_gap_percent(prev_close: f64, pre_open: f64) -> Option<f64> {
    if !(prev_close.is_finite() && pre_open.is_finite()) || prev_close <= 0.0 {
        return None;
    }
    Some((pre_open - prev_close) / prev_close * 100.0)
}

fn evaluate_status(gap_pct: f64) -> &'static str {
    if gap_pct >= GAP_THRESHOLD_PCT {
        "🚀 Strong Gap Up"
    } else if gap_pct <= -GAP_THRESHOLD_PCT {
        "⚠️ Sharp Gap Down"
    } else {
        "✅ Normal Open"
    }
}

async fn fetch_pre_market_data(
    client: &reqwest::Client,
    _api_key: &str,
) -> Result<Vec<StockSnapshot>, Box<dyn std::error::Error>> {
    // Health check WITHOUT leaking the credential:
    let health_res = client
        .get("https://httpbin.org/status/200")
        .send()
        .await?
        .error_for_status()?;

    if !health_res.status().is_success() {
        return Err(format!("API हेल्थ चेक फेल: HTTP {}", health_res.status()).into());
    }

    Ok(vec![
        StockSnapshot {
            symbol: "RELIANCE".into(),
            prev_close: 2950.0,
            pre_open_price: 2968.0,
        },
        StockSnapshot {
            symbol: "TCS".into(),
            prev_close: 4100.0,
            pre_open_price: 4195.0,
        },
        StockSnapshot {
            symbol: "HDFCBANK".into(),
            prev_close: 1650.0,
            pre_open_price: 1605.0,
        },
        StockSnapshot {
            symbol: "INFY".into(),
            prev_close: 1820.0,
            pre_open_price: 1826.5,
        },
    ])
}

/// Telegram Bot API के ज़रिए HTML फॉर्मेटेड मैसेज भेजता है
async fn send_telegram_report(
    client: &reqwest::Client,
    bot_token: &str,
    chat_id: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let payload = TelegramPayload {
        chat_id,
        text: message,
        parse_mode: "HTML",
    };

    let res = client.post(&url).json(&payload).send().await?;

    if !res.status().is_success() {
        let err_body = res.text().await.unwrap_or_default();
        return Err(format!("Telegram API एरर: {}", err_body).into());
    }

    println!("Telegram रिपोर्ट सफलतापूर्वक भेज दी गई!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("premarket-reporter/0.1")
        .build()?;
    let api_key = env::var("BROKER_API_KEY").unwrap_or_else(|_| "DEMO_KEY".to_string());

    let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let now_ist = Utc::now().with_timezone(&ist_offset);
    let timestamp = now_ist.format("%d-%m-%Y %I:%M %p IST").to_string();

    let stocks = fetch_pre_market_data(&client, &api_key).await?;

    // 1. GitHub Summary के लिए Markdown रिपोर्ट और Telegram के लिए HTML मैसेज साथ-साथ बनाएं
    let mut md_report = format!("# 📊 प्री-मार्केट डेटा रिपोर्ट ({})\n\n", timestamp);
    md_report.push_str("| स्टॉक | पिछला क्लोज़ (₹) | प्री-ओपन (₹) | गैप (%) | स्थिति |\n");
    md_report.push_str("| :--- | :---: | :---: | :---: | :--- |\n");

    let mut tg_msg = format!("<b>📊 प्री-मार्केट रिपोर्ट</b>\n<i>{}</i>\n\n", timestamp);

    let mut high_volatility_count = 0;

    for stock in &stocks {
        let Some(gap) = calculate_gap_percent(stock.prev_close, stock.pre_open_price) else {
            writeln!(
                md_report,
                "| **{}** | — | — | — | ❓ Invalid data |",
                stock.symbol
            )?;
            continue;
        };
        let status = evaluate_status(gap);
        if gap.abs() >= GAP_THRESHOLD_PCT {
            high_volatility_count += 1;
        }
        writeln!(
            md_report,
            "| **{}** | {:.2} | {:.2} | {:+.2}% | {} |",
            stock.symbol, stock.prev_close, stock.pre_open_price, gap, status
        )?;
        writeln!(
            tg_msg,
            "<b>{}</b>: ₹{:.2} → ₹{:.2} (<code>{:+.2}%</code>)\n{}\n",
            stock.symbol, stock.prev_close, stock.pre_open_price, gap, status
        )?;
    }

    let footer = format!(
        "कुल स्टॉक्स: {} | हाई-गैप अलर्ट्स: {}",
        stocks.len(),
        high_volatility_count
    );
    md_report.push_str(&format!("\n---\n**{}**\n", footer));
    tg_msg.push_str(&format!("━━━━━━━━━━━━━━━\n<b>{}</b>", footer));

    // 2. Markdown फ़ाइल सेव करें
    let reports_dir = Path::new("reports");
    fs::create_dir_all(reports_dir)?;
    fs::write(reports_dir.join("pre_market_report.md"), &md_report)?;

    // 3. अगर Telegram Secrets सेट हैं, तो मैसेज भेजें (लोकल टेस्टिंग में स्किप हो जाएगा)
    match (env::var("TELEGRAM_BOT_TOKEN"), env::var("TELEGRAM_CHAT_ID")) {
        (Ok(token), Ok(chat_id)) if !token.is_empty() && !chat_id.is_empty() => {
            send_telegram_report(&client, &token, &chat_id, &tg_msg).await?;
        }
        _ => {
            println!("Telegram Secrets नहीं मिले — नोटिफिकेशन स्किप किया गया।");
        }
    }

    Ok(())
}
