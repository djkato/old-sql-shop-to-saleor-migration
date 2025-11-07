pub const SQL_Endpoint: &str = dotenv!("DATABASE_URL");

use anyhow::Context;
use dotenvy_macro::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::cell::RefCell;
use std::path::Path;
use std::rc::{Rc, Weak};

pub async fn query_all() -> anyhow::Result<(Vec<Rc<RefCell<FinalCategory>>>, Vec<FinalProduct>)> {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(20)
        .connect(SQL_Endpoint)
        .await?;
    let products: Vec<Product> = sqlx::query_as!(Product, "SELECT * from products LIMIT 100000;")
        .fetch_all(&pool)
        .await?;

    let categories: Vec<Category> =
        sqlx::query_as!(Category, "SELECT * from categories LIMIT 100000;")
            .fetch_all(&pool)
            .await?;

    let category_product: Vec<CategoryProduct> = sqlx::query_as!(
        CategoryProduct,
        "SELECT * from category_product LIMIT 100000;"
    )
    .fetch_all(&pool)
    .await?;

    let mut final_categories = FinalCategory::from_categories(categories);
    let file_product: Vec<FileProduct> =
        sqlx::query_as!(FileProduct, "SELECT * FROM file_product LIMIT 100000;")
            .fetch_all(&pool)
            .await?;
    let files: Vec<File> = sqlx::query_as!(File, "SELECT* FROM files LIMIT 100000;")
        .fetch_all(&pool)
        .await?;

    //dbg!("{}", final_categories.get(20));
    let final_products = FinalProduct::from_products(
        products,
        &final_categories,
        category_product,
        file_product,
        files,
    );

    //So I can see how new products fare first
    //final_products = final_products.into_iter().rev().collect();

    //sort categories by level
    final_categories.sort_by(|a, b| {
        let mut depth_a = 0;
        let mut depth_b = 0;
        for (mut prev_parent, depth) in [(a.clone(), &mut depth_a), (b.clone(), &mut depth_b)] {
            loop {
                if let Some(curr_parent) = prev_parent.clone().borrow().parent_category.clone() {
                    prev_parent = curr_parent;
                    *depth += 1;
                } else {
                    break;
                }
            }
        }
        depth_a.cmp(&depth_b)
    });

    Ok((final_categories, final_products))
}

pub struct File {
    id: u32,
    name: String,
    mime_type: String,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub struct FileProduct {
    product_id: u32,
    file_id: u32,
}

#[derive(Debug)]
pub struct FinalProduct {
    pub product: Product,
    pub saleor_id: Option<cynic::Id>,
    pub category: Option<Rc<RefCell<FinalCategory>>>,
    pub images: Vec<String>,
    pub price: Option<String>,
    pub SKU: String,
    pub slug: String,
}

impl FinalProduct {
    pub fn from_products(
        products: Vec<Product>,
        categories: &Vec<Rc<RefCell<FinalCategory>>>,
        rel_category_product: Vec<CategoryProduct>,
        file_products: Vec<FileProduct>,
        files: Vec<File>,
    ) -> Vec<Self> {
        let slugify = regex::Regex::new(r###"[^a-zA-Z0-9-]+"###).unwrap();
        //filter out products with no name or test names or deleted while mapping
        let mut final_products: Vec<FinalProduct> = products
            .into_iter()
            .filter_map(|product| {
                if product.name.is_empty()
                    || product.name.contains("test")
                    || product.deleted_at.is_some()
                {
                    return None;
                }
                let sku = product.code.clone();
                let slug = slugify
                    .replace_all(
                        deunicode::deunicode(product.name.as_str())
                            .to_ascii_lowercase()
                            .trim(),
                        "-",
                    )
                    .to_string();

                Some(FinalProduct {
                    product,
                    saleor_id: None,
                    category: None,
                    images: Vec::new(),
                    price: None,
                    SKU: sku,
                    slug,
                })
            })
            .collect();

        //Try to find a category product belongs to, then get pointer to it
        for product in final_products.iter_mut() {
            let category_match = rel_category_product
                .iter()
                .find(|rel| rel.product_id == product.product.id);

            if let Some(category) = category_match {
                product.category = categories
                    .iter()
                    .cloned()
                    .find(|cat| cat.borrow().category.borrow().id == category.category_id as u32);
            }
        }
        //Find the price a product belongs to
        for product in final_products.iter_mut() {
            if let Some(price) = &product.product.retail_price_with_iva {
                product.price = Some(price.to_string())
            }
            //fix names
            // product.product.name = gql_queries::Jsonstring::purify_old_json(&product.product.name);

            //Fix descriptions
            product.product.description =
                gql_queries::Jsonstring::purify_old_json(&product.product.description);
            product.product.short_description =
                gql_queries::Jsonstring::purify_old_json(&product.product.short_description);
        }

        // Find images belonging to the product
        for file_product in file_products {
            if let Some(match_product) = final_products
                .iter_mut()
                .find(|f| f.product.id == file_product.product_id)
            {
                if let Some(file_match) = files.iter().find(|file| file.id == file_product.file_id)
                {
                    let mut file_name = file_match.name.clone();
                    if file_match.name.contains(".webp") {
                        if let Some(name) = file_match.name.rsplit_once(".") {
                            let name = name.0.to_ascii_lowercase();
                            if name.contains(".png")
                                || name.contains(".jpg")
                                || name.contains(".jpeg")
                            {
                                file_name = name
                            }
                        }
                    }
                    let ip_address = local_ip_address::local_ip()
                        .expect("Failed finding local IP. Are you offline?");
                    let file_name = format!(
                        "http://{}:38008/products/{}",
                        ip_address.to_string(),
                        file_name
                    );
                    match_product.images.push(file_name);
                }
            }
        }

        // Avoid clashing SKUs by creating a new norm, aka appending 3 more numbers
        let mut skus: Vec<SKU> = Vec::new();
        for product in final_products.iter_mut() {
            if let Some(matching_sku) = skus.iter_mut().find(|s| s.base_sku == product.SKU) {
                matching_sku.latest_sku_suffix = matching_sku.latest_sku_suffix + 1;
                product.SKU = product.SKU.clone()
                    + format!(" {:0>3}", matching_sku.latest_sku_suffix).as_str();
            } else {
                let new_sku = SKU {
                    base_sku: product.SKU.clone(),
                    latest_sku_suffix: 1,
                };
                product.SKU = product.SKU.clone() + " 001";
                skus.push(new_sku);
            }
        }
        final_products
    }
}

pub struct SKU {
    pub base_sku: String,
    pub latest_sku_suffix: u16,
}
#[derive(Debug)]
pub struct FinalCategory {
    pub me: Weak<RefCell<Self>>,
    pub category: Rc<RefCell<Category>>,
    pub parent_category: Option<Rc<RefCell<FinalCategory>>>,
    pub saleor_id: Option<cynic::Id>,
    pub images: Vec<Box<Path>>,
}

impl FinalCategory {
    pub fn new(category: Category) -> Rc<RefCell<Self>> {
        Rc::new_cyclic(|me| {
            RefCell::new(FinalCategory {
                me: me.clone(),
                category: Rc::new(RefCell::new(category)),
                parent_category: None,
                saleor_id: None,
                images: Vec::new(),
            })
        })
    }
    pub fn new_self_rc(&self) -> Rc<RefCell<Self>> {
        self.me
            .upgrade()
            .context("Failed to upgrade pointer to FinalCategory")
            .unwrap()
    }

    pub fn from_categories(categories: Vec<Category>) -> Vec<Rc<RefCell<Self>>> {
        //filter out empty named, "" named or test named
        //CHECK FOR DATE OF DELTETION: IF ANY, FILTER IT OUT
        let mut final_categories: Vec<Rc<RefCell<FinalCategory>>> = categories
            .into_iter()
            .filter_map(|cat| {
                if cat.name.is_empty()
                    || cat.name.to_lowercase().contains("test")
                    || cat.deleted_at.is_some()
                {
                    return None;
                }
                //Remove non-relevant categories
                match cat.name.as_str() {
                    "Root" => return None,
                    "Vianočné dekorácie" => return None,
                    "Veľkonočné dekorácie" => return None,
                    "Roľničky kovové" => return None,
                    "Dekorácia zápich" => return None,
                    "Aplikácie so zapínaním" => return None,
                    "Ozdoby sisalové" => return None,
                    "Girlandy" => return None,
                    "Ozdoby na zavesenie" => return None,
                    "Dekoračné predmety" => return None,
                    "Aplikácie s magnetom" => return None,
                    "Aplikácie na drôtiku" => return None,
                    "Kategórie" => return None,
                    _ => {}
                }
                Some(FinalCategory::new(cat))
            })
            .collect();
        let temp_final_categories = final_categories.clone();
        // dbg!(final_categories[3].borrow().category.borrow());
        for category in final_categories.iter_mut() {
            //pair categories to sub categories etc
            let find_res = temp_final_categories.iter().find(|cat| {
                cat.borrow().category.borrow().id
                    == category
                        .borrow()
                        .category
                        .borrow()
                        .parent_id
                        .unwrap_or(u32::MAX)
            });

            if let Some(parent_category) = find_res {
                category.borrow_mut().parent_category = Some(parent_category.clone());
            }
            // // Undo jsons in name
            // if let Ok(new_name) = serde_json::from_str::<OldJson>(
            //     category.borrow_mut().category.borrow_mut().name.as_str(),
            // ) {
            //     category.borrow_mut().category.borrow_mut().name = new_name.sk;
            // }
            // // Undo jsons in slug
            // if let Ok(new_slug) = serde_json::from_str::<OldJson>(
            //     category.borrow_mut().category.borrow_mut().slug.as_str(),
            // ) {
            //     category.borrow_mut().category.borrow_mut().slug = new_slug.sk;
            // }
            // Change description Json from old to new editorJS
            // Some descriptions are HTML <p> elements, some are div<span... try to parse them
            let cat = category.borrow_mut();
            let mut cat_cat = cat.category.borrow_mut();
            cat_cat.description =
                gql_queries::Jsonstring::parse_old_json(&cat_cat.description).to_string();
        }

        final_categories
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OldJson {
    pub sk: String,
}
#[derive(sqlx::FromRow, Debug)]
pub struct ProductPrices {
    pub country_id: String,
    pub price: BigDecimal,
    pub price_with_vat: BigDecimal,
    pub product_id: u32,
    pub recommended_price: Option<BigDecimal>,
    pub recommended_price_with_vat: Option<BigDecimal>,
    pub store_id: u32,
    pub wholesale_price: BigDecimal,
    pub wholesale_price_with_vat: BigDecimal,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct CategoryProduct {
    pub id: u32,
    pub category_id: u32,
    pub product_id: u32,
}

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct Category {
    pub id: u32,
    pub name: String,
    pub parent_id: Option<u32>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub slug: String,
    pub description: String,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub keywords: Option<String>,
    pub image_id: Option<u32>,
    #[sqlx(default)]
    pub translation: Option<Vec<u8>>,
    pub titleHelp: String,
    pub glami_id: Option<i32>,
    pub heureka_id: Option<i32>,
    pub favi_id: Option<i32>,
    pub isRoot: i8,
    pub mall_id: Option<String>,
    pub is_active: i8,
    pub sort_id: i32,
    pub discount: Option<f64>,
    pub ebay_id: Option<i32>,
    pub amazon_id: Option<i32>,
}
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Product {
    pub id: u32,
    pub name: String,
    pub short_description: String,
    pub description: String,
    pub image_id: Option<u32>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub id_language: u32,
    pub code: String,
    pub wholesale_price: Option<BigDecimal>,
    pub retail_price: Option<BigDecimal>,
    pub wholesale_price_with_iva: Option<BigDecimal>,
    pub retail_price_with_iva: Option<BigDecimal>,
    pub quantity: Option<i32>,
    pub unit_id: Option<u32>,
    pub discount: i32,
    pub status: String,
    #[sqlx(default)]
    pub translation: Option<Vec<u8>>,
    pub availability_text_id: Option<i32>,
    pub weight: Option<f64>,
    pub amazon: Option<i32>,
    pub ebay: Option<i32>,
    pub mall: Option<i32>,
}
#[derive(sqlx::Type, Clone, Copy, Serialize, Deserialize, Debug)]
#[sqlx(type_name = "status", rename_all = "lowercase")]
pub enum Status {
    Available,
    Ended,
    Arrival,
}

#[derive(Debug, Clone)]
pub struct Jsonstring(pub String);

impl std::fmt::Display for Jsonstring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Jsonstring {
    pub fn from_string(text: String) -> Self {
        use rand::distributions::{Alphanumeric, DistString};
        use serde_json::json;
        let id = Alphanumeric.sample_string(&mut rand::thread_rng(), 8);
        let dt = chrono::offset::Local::now().timestamp();
        let json = json!(
        {
            "time": dt,
            "blocks": [
                {
                "id": id,
                "type": "paragraph",
                "data": {
                    "text": text
                }
                }
            ],
            "version": "2.24.3"
        }
                );
        Jsonstring(json.to_string())
    }
    pub fn purify_old_json(text: &String) -> String {
        let mut new_text = text.to_owned();
        //unwrap all the jsons..
        if let Ok(old_json_text) = serde_json::from_str::<OldJson>(text.as_str()) {
            let mut raw = old_json_text.sk;
            loop {
                if !raw.is_empty() {
                    if let Ok(next_sk) = serde_json::from_str::<OldJson>(raw.as_str()) {
                        raw = next_sk.sk;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            new_text = raw;
        }
        /* --- LET'S TRY SENDING THE HTML MARKUP INSIDE STUFF
        //replace all <br /> with \n
        new_text = new_text.replace("<br />", "\n");

        //delete all teh html syntax around text
        let mut i = 0;
        loop {
            i += 1;
            if i > 10000 {
                println!("looping still..{i}");
                println!("{text}");
            }
            if let Some(html_start) = new_text.find('<').or(new_text.find("</")) {
                if let Some(html_end) = new_text.find('>').or(new_text.find("/>")) {
                    if &new_text[html_end..html_end + 1] == ">" {
                        new_text.drain(html_start..html_end + 1);
                    } else if &new_text[html_end..html_end + 2] == "/>" {
                        new_text.drain(html_start..html_end + 2);
                    }
                } else {
                    new_text = new_text.replace('<', "");
                    new_text = new_text.replace("</", "");
                }
            } else {
                break;
            }
        }
        */
        //un-escape html symbols
        new_text = html_escape::decode_html_entities(&new_text).to_string();
        new_text
    }

    pub fn parse_old_json(text: &String) -> Jsonstring {
        Self::from_string(Self::purify_old_json(text))
    }
}
