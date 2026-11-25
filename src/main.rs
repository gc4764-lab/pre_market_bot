use chrono::{FixedOffset, Utc};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct StockSnapshot {
    symbol: String,
    prev_close: f64,
    pre_open_price: f64,
}

/// गैप प्रतिशत (Gap %) की गणना
fn calculate_gap_percent(prev_close: f64, pre_open: f64) -> f64 {
    ((pre_open - prev_close) / prev_close) * 100.0
}

/// प्री-मार्केट रिस्क स्टेटस तय करना
fn evaluate_status(gap_pct: f64) -> &'static str {
    if gap_pct >= 2.0 {
        "🚀 Strong Gap Up (Avoid FOMO)"
    } else if gap_pct <= -2.0 {
        "⚠️ Sharp Gap Down (Check Support)"
    } else {
        "✅ Normal Open (Ready for Setup)"
    }
}

/// ब्रोकर/डेटा API से प्री-मार्केट डेटा लाना
/// (उदाहरण के लिए यहाँ API चेक + डेमो डेटा दिया गया है; आप इसे अपने ब्रोकर के असली एंडपॉइंट से बदल सकते हैं)
async fn fetch_pre_market_data(api_key: &str) -> Result<Vec<StockSnapshot>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    // उदाहरण: API की हेल्थ और नेटवर्क कनेक्टिविटी जांचना
    let health_res = client
        .get("https://httpbin.org/status/200")
        .bearer_auth(api_key)
        .send()
        .await?;

    if !health_res.status().is_success() {
        return Err(format!("API हेल्थ चेक फेल: HTTP {}", health_res.status()).into());
    }

    // असली प्रोडक्शन में यहाँ `health_res.json::<Vec<StockSnapshot>>().await?` आएगा
    Ok(vec![
        StockSnapshot { symbol: "RELIANCE".into(), prev_close: 2950.0, pre_open_price: 2968.0 },
        StockSnapshot { symbol: "TCS".into(),      prev_close: 4100.0, pre_open_price: 4195.0 },
        StockSnapshot { symbol: "HDFCBANK".into(), prev_close: 1650.0, pre_open_price: 1605.0 },
        StockSnapshot { symbol: "INFY".into(),     prev_close: 1820.0, pre_open_price: 1826.5 },
    ])
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. GitHub Secret से API Key पढ़ें (लोकल टेस्ट के लिए डिफ़ॉल्ट वैल्यू)
    let api_key = env::var("BROKER_API_KEY").unwrap_or_else(|_| "DEMO_KEY".to_string());

    // 2. भारतीय समय (IST = UTC + 5:30) निकालें
    let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let now_ist = Utc::now().with_timezone(&ist_offset);
    let timestamp = now_ist.format("%d-%m-%Y %I:%M %p IST").to_string();

    println!("प्री-मार्केट चेक शुरू: {}", timestamp);

    // 3. डेटा फेच करें
    let stocks = fetch_pre_market_data(&api_key).await?;

    // 4. Markdown रिपोर्ट तैयार करें
    let mut report = String::new();
    report.push_str(&format!("# 📊 प्री-मार्केट डेटा रिपोर्ट ({})\n\n", timestamp));
    report.push_str("| स्टॉक (Symbol) | पिछला क्लोज़ (₹) | प्री-ओपन (₹) | गैप (%) | स्थिति (Status) |\n");
    report.push_str("| :--- | :---: | :---: | :---: | :--- |\n");

    let mut high_volatility_count = 0;

    for stock in &stocks {
        let gap = calculate_gap_percent(stock.prev_close, stock.pre_open_price);
        let status = evaluate_status(gap);
        if gap.abs() >= 2.0 {
            high_volatility_count += 1;
        }

        report.push_str(&format!(
            "| **{}** | {:.2} | {:.2} | {:+.2}% | {} |\n",
            stock.symbol, stock.prev_close, stock.pre_open_price, gap, status
        ));
    }

    report.push_str("\n---\n");
    report.push_str(&format!(
        "**कुल जांचे गए स्टॉक्स:** {} | **हाई-गैप अलर्ट्स:** {}\n",
        stocks.len(),
        high_volatility_count
    ));

    // 5. रिपोर्ट को फाइल में सेव करें
    let reports_dir = Path::new("reports");
    fs::create_dir_all(reports_dir)?;
    let report_path = reports_dir.join("pre_market_report.md");
    fs::write(&report_path, &report)?;

    println!("रिपोर्ट सफलतापूर्वक सेव हो गई: {:?}", report_path);
    Ok(())
}
