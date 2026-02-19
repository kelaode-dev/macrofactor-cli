use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone};
use clap::{Parser, Subcommand};
use macro_factor_api::client::MacroFactorClient;
use macro_factor_api::models::SearchFoodResult;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
    /// Log a food entry (quick add)
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
        /// Time in HH:MM format (default: now)
        #[arg(long)]
        time: Option<String>,
    },
    /// Search the food database
    SearchFood {
        /// Search query
        query: String,
    },
    /// Log a food from the last search results
    LogSearchedFood {
        #[arg(long)]
        date: NaiveDate,
        /// Index from search results (1-based)
        #[arg(long)]
        food_index: usize,
        /// Serving index (1-based, default: 1 = default serving)
        #[arg(long, default_value = "1")]
        serving: usize,
        /// Quantity of servings (default: 1.0)
        #[arg(long, default_value = "1.0")]
        quantity: f64,
        /// Time in HH:MM format (default: now)
        #[arg(long)]
        time: Option<String>,
    },
    /// Delete a food entry
    DeleteFood {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long)]
        entry_id: String,
    },
    /// Delete a weight entry
    DeleteWeight {
        #[arg(long)]
        date: NaiveDate,
    },
    /// Sync daily nutrition totals
    SyncDay {
        #[arg(long)]
        date: NaiveDate,
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
    /// Log a nutrition summary (manual import)
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

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("macrofactor-cli")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

fn search_cache_path() -> PathBuf {
    config_dir().join("last-search.json")
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

/// Parse --time HH:MM and combine with date into DateTime<Local>, or use now.
fn make_logged_at(date: NaiveDate, time: &Option<String>) -> Result<chrono::DateTime<Local>> {
    match time {
        Some(t) => {
            let parts: Vec<&str> = t.split(':').collect();
            if parts.len() != 2 {
                anyhow::bail!("--time must be in HH:MM format");
            }
            let hour: u32 = parts[0].parse()?;
            let minute: u32 = parts[1].parse()?;
            let naive = date.and_time(NaiveTime::from_hms_opt(hour, minute, 0)
                .ok_or_else(|| anyhow::anyhow!("Invalid time"))?);
            Ok(Local.from_local_datetime(&naive).single()
                .ok_or_else(|| anyhow::anyhow!("Ambiguous local time"))?)
        }
        None => {
            let now = Local::now();
            if date == now.date_naive() {
                Ok(now)
            } else {
                let naive = date.and_hms_opt(12, 0, 0).unwrap();
                Ok(Local.from_local_datetime(&naive).single()
                    .ok_or_else(|| anyhow::anyhow!("Ambiguous local time"))?)
            }
        }
    }
}

fn save_search_cache(results: &[SearchFoodResult]) -> Result<()> {
    let path = search_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string(results)?)?;
    Ok(())
}

fn load_search_cache() -> Result<Vec<SearchFoodResult>> {
    let path = search_cache_path();
    let data = fs::read_to_string(&path)
        .with_context(|| "No search results cached. Run `search-food` first.")?;
    serde_json::from_str(&data).context("Invalid search cache")
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login { email, password } => {
            let firebase_key = "AIzaSyA17Uwy37irVEQSwz6PIyX3wnkHrDBeleA";
            let url = format!(
                "https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key={}",
                firebase_key
            );
            let http = reqwest::Client::new();
            let resp = http.post(&url)
                .header("X-Ios-Bundle-Identifier", "com.sbs.diet")
                .json(&json!({
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
                println!("{}", json!({"status": "ok", "message": "Logged in successfully"}));
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
                        println!("  [{}] {} ({}) — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f | {:.0}g  [id: {}]",
                            time,
                            f.name.as_deref().unwrap_or("Unknown"),
                            f.brand.as_deref().unwrap_or(""),
                            f.calories().unwrap_or(0.0),
                            f.protein().unwrap_or(0.0),
                            f.carbs().unwrap_or(0.0),
                            f.fat().unwrap_or(0.0),
                            f.weight_grams().unwrap_or(0.0),
                            f.entry_id,
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

        Commands::LogFood { date, name, calories, protein, carbs, fat, time } => {
            let mut client = get_client()?;
            let logged_at = make_logged_at(date, &time)?;
            client.log_food(logged_at, &name, calories, protein, carbs, fat).await?;

            if cli.json {
                println!("{}", json!({"status": "ok", "message": "Food logged"}));
            } else {
                println!("✓ Logged '{}' on {} — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f",
                    name, date, calories, protein, carbs, fat);
            }
        }

        Commands::SearchFood { query } => {
            let client = get_client()?;
            let results = client.search_foods(&query).await?;

            if results.is_empty() {
                if cli.json {
                    println!("[]");
                } else {
                    println!("No results for '{}'", query);
                }
                return Ok(());
            }

            // Cache results for log-searched-food
            save_search_cache(&results)?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                println!("── Search Results for '{}' ({} results) ──\n", query, results.len());
                for (i, r) in results.iter().enumerate() {
                    let brand = r.brand.as_deref().unwrap_or("");
                    let brand_str = if brand.is_empty() { String::new() } else { format!(" ({})", brand) };
                    let src = if r.branded { "branded" } else { "common" };

                    // Show per default serving if available, otherwise per 100g
                    let (cal, p, c, f, serving_info) = if let Some(ref ds) = r.default_serving {
                        let scale = ds.gram_weight / 100.0;
                        (
                            r.calories_per_100g * scale,
                            r.protein_per_100g * scale,
                            r.carbs_per_100g * scale,
                            r.fat_per_100g * scale,
                            format!("per {} ({:.0}g)", ds.description, ds.gram_weight),
                        )
                    } else {
                        (
                            r.calories_per_100g,
                            r.protein_per_100g,
                            r.carbs_per_100g,
                            r.fat_per_100g,
                            "per 100g".to_string(),
                        )
                    };

                    println!("  {:>2}. {}{} [{}]", i + 1, r.name, brand_str, src);
                    println!("      {:.0} kcal | {:.0}p / {:.0}c / {:.0}f  ({})", cal, p, c, f, serving_info);

                    if r.servings.len() > 1 {
                        let serving_list: Vec<String> = r.servings.iter()
                            .map(|s| format!("{} ({:.0}g)", s.description, s.gram_weight))
                            .collect();
                        println!("      servings: {}", serving_list.join(", "));
                    }
                    println!();
                }
            }
        }

        Commands::LogSearchedFood { date, food_index, serving, quantity, time } => {
            let results = load_search_cache()?;
            if food_index == 0 || food_index > results.len() {
                anyhow::bail!("Invalid food index {}. Last search had {} results.", food_index, results.len());
            }
            let food = &results[food_index - 1];

            // Determine serving
            let food_serving = if serving == 1 {
                // Use default serving, falling back to first available or 100g
                food.default_serving.clone()
                    .or_else(|| food.servings.first().cloned())
                    .unwrap_or_else(|| macro_factor_api::models::FoodServing {
                        description: "100g".to_string(),
                        amount: 1.0,
                        gram_weight: 100.0,
                    })
            } else {
                let idx = serving - 1;
                if idx >= food.servings.len() {
                    anyhow::bail!("Invalid serving index {}. Food has {} servings.", serving, food.servings.len());
                }
                food.servings[idx].clone()
            };

            let mut client = get_client()?;
            let logged_at = make_logged_at(date, &time)?;
            client.log_searched_food(logged_at, food, &food_serving, quantity).await?;

            let scale = food_serving.gram_weight / 100.0 * quantity;
            if cli.json {
                println!("{}", json!({
                    "status": "ok",
                    "message": "Searched food logged",
                    "food": food.name,
                    "serving": food_serving.description,
                    "quantity": quantity,
                }));
            } else {
                println!("✓ Logged '{}' on {} — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f ({:.1}x {})",
                    food.name, date,
                    food.calories_per_100g * scale,
                    food.protein_per_100g * scale,
                    food.carbs_per_100g * scale,
                    food.fat_per_100g * scale,
                    quantity, food_serving.description,
                );
            }
        }

        Commands::DeleteFood { date, entry_id } => {
            let mut client = get_client()?;
            client.delete_food_entry(date, &entry_id).await?;

            if cli.json {
                println!("{}", json!({"status": "ok", "message": "Food entry deleted"}));
            } else {
                println!("✓ Deleted food entry {} on {}", entry_id, date);
            }
        }

        Commands::DeleteWeight { date } => {
            let mut client = get_client()?;
            client.delete_weight_entry(date).await?;

            if cli.json {
                println!("{}", json!({"status": "ok", "message": "Weight entry deleted"}));
            } else {
                println!("✓ Deleted weight entry on {}", date);
            }
        }

        Commands::SyncDay { date } => {
            let mut client = get_client()?;
            client.sync_day(date).await?;

            if cli.json {
                println!("{}", json!({"status": "ok", "message": "Day synced"}));
            } else {
                println!("✓ Synced daily totals for {}", date);
            }
        }

        Commands::LogWeight { date, weight, body_fat } => {
            let mut client = get_client()?;
            client.log_weight(date, weight, body_fat).await?;

            if cli.json {
                println!("{}", json!({"status": "ok", "message": "Weight logged"}));
            } else {
                let bf = body_fat.map(|v| format!(" ({}% bf)", v)).unwrap_or_default();
                println!("✓ Logged {:.1} kg{} on {}", weight, bf, date);
            }
        }

        Commands::LogNutrition { date, calories, protein, carbs, fat } => {
            let mut client = get_client()?;
            client.log_nutrition(date, calories, Some(protein), Some(carbs), Some(fat)).await?;

            if cli.json {
                println!("{}", json!({"status": "ok", "message": "Nutrition logged"}));
            } else {
                println!("✓ Logged nutrition on {} — {:.0} kcal | {:.0}p / {:.0}c / {:.0}f",
                    date, calories, protein, carbs, fat);
            }
        }
    }

    Ok(())
}
