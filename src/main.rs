use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use clap::{Parser, Subcommand};
use macro_factor_api::client::MacroFactorClient;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "macrofactor-cli", about = "CLI for MacroFactor nutrition tracking")]
struct Cli {
    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate and save refresh token
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
    },
    /// Show user profile
    Profile,
    /// Show current calorie/macro targets and TDEE
    Goals,
    /// Daily nutrition summaries
    Nutrition {
        #[arg(long)]
        start: Option<NaiveDate>,
        #[arg(long)]
        end: Option<NaiveDate>,
    },
    /// Food entries for a day
    FoodLog {
        #[arg(long)]
        date: Option<NaiveDate>,
    },
    /// Weight entries
    Weight {
        #[arg(long)]
        start: Option<NaiveDate>,
        #[arg(long)]
        end: Option<NaiveDate>,
    },
    /// Step counts
    Steps {
        #[arg(long)]
        start: Option<NaiveDate>,
        #[arg(long)]
        end: Option<NaiveDate>,
    },
    /// Log a food entry
    LogFood {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long)]
        name: String,
        #[arg(long)]
        calories: f64,
        #[arg(long)]
        protein: f64,
        #[arg(long)]
        carbs: f64,
        #[arg(long)]
        fat: f64,
    },
    /// Log a weight entry
    LogWeight {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long)]
        weight: f64,
        #[arg(long)]
        body_fat: Option<f64>,
    },
    /// Log a nutrition summary
    LogNutrition {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long)]
        calories: f64,
        #[arg(long)]
        protein: f64,
        #[arg(long)]
        carbs: f64,
        #[arg(long)]
        fat: f64,
    },
}

#[derive(Serialize, Deserialize)]
struct Config {
    refresh_token: String,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("macrofactor-cli")
        .join("config.json")
}

fn load_config() -> Result<Config> {
    let path = config_path();
    let data = fs::read_to_string(&path)
        .with_context(|| "Not logged in. Run `macrofactor-cli login` first.")?;
    serde_json::from_str(&data).context("Invalid config file")
}

fn save_config(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

fn get_client() -> Result<MacroFactorClient> {
    let config = load_config()?;
    Ok(MacroFactorClient::new(config.refresh_token))
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn seven_days_ago() -> NaiveDate {
    today() - chrono::Duration::days(7)
}

fn day_name(idx: usize) -> &'static str {
    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        .get(idx)
        .unwrap_or(&"?")
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login { email, password } => {
            // Use Firebase REST API directly to get the refresh token
            let firebase_key = "AIzaSyA17Uwy37irVEQSwz6PIyX3wnkHrDBeleA";
            let url = format!(
                "https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key={}",
                firebase_key
            );
            let http = reqwest::Client::new();
            let resp = http.post(&url)
                .header("X-Ios-Bundle-Identifier", "com.sbs.diet")
                .json(&serde_json::json!({
                    "email": email,
                    "password": password,
                    "returnSecureToken": true
                }))
                .send().await?;

            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Login failed: {}", body);
            }

            let body: serde_json::Value = resp.json().await?;
            let refresh_token = body["refreshToken"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No refresh token in response"))?;

            save_config(&Config { refresh_token: refresh_token.to_string() })?;

            if cli.json {
                println!("{}", serde_json::json!({"status": "ok", "message": "Logged in successfully"}));
            } else {
                println!("✓ Logged in successfully. Config saved to {:?}", config_path());
            }
        }

        Commands::Profile => {
            let mut client = get_client()?;
            let profile = client.get_profile().await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&profile)?);
            } else {
                println!("── Profile ──");
                if let Some(obj) = profile.as_object() {
                    for (k, v) in obj {
                        if k == "planner" { continue; }
                        println!("  {}: {}", k, v);
                    }
                }
            }
        }

        Commands::Goals => {
            let mut client = get_client()?;
            let goals = client.get_goals().await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&goals)?);
            } else {
                println!("── Goals ──");
                if let Some(tdee) = goals.tdee {
                    println!("  TDEE: {:.0} kcal", tdee);
                }
                if let Some(ref style) = goals.program_style {
                    println!("  Program: {} / {}", style, goals.program_type.as_deref().unwrap_or("—"));
                }
                let dow = today().weekday().num_days_from_monday() as usize;
                println!("\n  Today ({}):", day_name(dow));
                if let Some(c) = goals.calories.get(dow) { println!("    Calories: {:.0} kcal", c); }
                if let Some(p) = goals.protein.get(dow) { println!("    Protein:  {:.0} g", p); }
                if let Some(c) = goals.carbs.get(dow) { println!("    Carbs:    {:.0} g", c); }
                if let Some(f) = goals.fat.get(dow) { println!("    Fat:      {:.0} g", f); }

                println!("\n  Weekly targets:");
                for i in 0..7 {
                    let cal = goals.calories.get(i).map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into());
                    let pro = goals.protein.get(i).map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into());
                    let carb = goals.carbs.get(i).map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into());
                    let fat = goals.fat.get(i).map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into());
                    println!("    {}: {} kcal | {}p / {}c / {}f", day_name(i), cal, pro, carb, fat);
                }
            }
        }

        Commands::Nutrition { start, end } => {
            let mut client = get_client()?;
            let s = start.unwrap_or_else(today);
            let e = end.unwrap_or_else(today);
            let entries = client.get_nutrition(s, e).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                if entries.is_empty() {
                    println!("No nutrition data for {} to {}", s, e);
                } else {
                    println!("── Nutrition ({} → {}) ──", s, e);
                    for n in &entries {
                        println!("  {}:  {} kcal | {}p / {}c / {}f | sugar: {} | fiber: {}",
                            n.date,
                            n.calories.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                            n.protein.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                            n.carbs.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                            n.fat.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                            n.sugar.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                            n.fiber.map(|v| format!("{:.0}", v)).unwrap_or_else(|| "—".into()),
                        );
                    }
                }
            }
        }

        Commands::FoodLog { date } => {
            let mut client = get_client()?;
            let d = date.unwrap_or_else(today);
            let entries = client.get_food_log(d).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                if entries.is_empty() {
                    println!("No food entries for {}", d);
                } else {
                    println!("── Food Log ({}) ──", d);
                    for f in &entries {
                        let time = format!("{}:{:02}",
                            f.hour.as_deref().unwrap_or("?"),
                            f.minute.as_deref().unwrap_or("0").parse::<u32>().unwrap_or(0));
                        println!("  [{}] {} ({}) — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f | {:.0}g",
                            time,
                            f.name.as_deref().unwrap_or("Unknown"),
                            f.brand.as_deref().unwrap_or(""),
                            f.calories().unwrap_or(0.0),
                            f.protein().unwrap_or(0.0),
                            f.carbs().unwrap_or(0.0),
                            f.fat().unwrap_or(0.0),
                            f.weight_grams().unwrap_or(0.0),
                        );
                    }
                }
            }
        }

        Commands::Weight { start, end } => {
            let mut client = get_client()?;
            let s = start.unwrap_or_else(seven_days_ago);
            let e = end.unwrap_or_else(today);
            let entries = client.get_weight_entries(s, e).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                if entries.is_empty() {
                    println!("No weight entries for {} to {}", s, e);
                } else {
                    println!("── Weight ({} → {}) ──", s, e);
                    for w in &entries {
                        let bf = w.body_fat.map(|v| format!(" ({}% bf)", v)).unwrap_or_default();
                        println!("  {}:  {:.1} kg{}", w.date, w.weight, bf);
                    }
                }
            }
        }

        Commands::Steps { start, end } => {
            let mut client = get_client()?;
            let s = start.unwrap_or_else(seven_days_ago);
            let e = end.unwrap_or_else(today);
            let entries = client.get_steps(s, e).await?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                if entries.is_empty() {
                    println!("No step data for {} to {}", s, e);
                } else {
                    println!("── Steps ({} → {}) ──", s, e);
                    for st in &entries {
                        println!("  {}:  {} steps", st.date, st.steps);
                    }
                }
            }
        }

        Commands::LogFood { date, name, calories, protein, carbs, fat } => {
            let mut client = get_client()?;
            client.log_food(date, &name, calories, protein, carbs, fat).await?;

            if cli.json {
                println!("{}", serde_json::json!({"status": "ok", "message": "Food logged"}));
            } else {
                println!("✓ Logged '{}' on {} — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f",
                    name, date, calories, protein, carbs, fat);
            }
        }

        Commands::LogWeight { date, weight, body_fat } => {
            let mut client = get_client()?;
            client.log_weight(date, weight, body_fat).await?;

            if cli.json {
                println!("{}", serde_json::json!({"status": "ok", "message": "Weight logged"}));
            } else {
                let bf = body_fat.map(|v| format!(" ({}% bf)", v)).unwrap_or_default();
                println!("✓ Logged {:.1} kg{} on {}", weight, bf, date);
            }
        }

        Commands::LogNutrition { date, calories, protein, carbs, fat } => {
            let mut client = get_client()?;
            client.log_nutrition(date, calories, Some(protein), Some(carbs), Some(fat)).await?;

            if cli.json {
                println!("{}", serde_json::json!({"status": "ok", "message": "Nutrition logged"}));
            } else {
                println!("✓ Logged nutrition on {} — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f",
                    date, calories, protein, carbs, fat);
            }
        }
    }

    Ok(())
}
