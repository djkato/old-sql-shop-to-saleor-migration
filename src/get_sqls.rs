use crate::gql_queries::{self, ProductType};

use anyhow::Context;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct QueryAllResult {
    pub categories: Vec<Rc<RefCell<FinalCategory>>>,
    pub products: Vec<FinalProduct>,
    pub product_types: Vec<Rc<RefCell<FinalProductType>>>,
}
pub async fn query_all() -> anyhow::Result<QueryAllResult> {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(20)
        .connect(gql_queries::SQL_Endpoint)
        .await?;

    //INFO: MAGIC NUMBER!
    let products: Vec<Product> = sqlx::query_as!(Product, "SELECT * from products LIMIT 100000;")
        .fetch_all(&pool)
        .await?;

    let categories: Vec<Category> =
            //INFO: MAGIC NUMBER!
        sqlx::query_as!(Category, "SELECT * from categories LIMIT 100000;")
            .fetch_all(&pool)
            .await?;

    let category_product: Vec<CategoryProduct> = sqlx::query_as!(
        CategoryProduct,
        //INFO: MAGIC NUMBER!
        "SELECT * from category_product LIMIT 100000;"
    )
    .fetch_all(&pool)
    .await?;

    let file_product: Vec<FileProduct> =
            //INFO: MAGIC NUMBER!
        sqlx::query_as!(FileProduct, "SELECT * FROM file_product LIMIT 100000;")
            .fetch_all(&pool)
            .await?;

    //INFO: MAGIC NUMBER!
    let files: Vec<File> = sqlx::query_as!(File, "SELECT* FROM files LIMIT 100000;")
        .fetch_all(&pool)
        .await?;

    let mut final_product_types = vec![];
    let mut final_categories =
        FinalCategory::from_categories(categories, files.clone(), &mut final_product_types).await;

    //dbg!("{}", final_categories.get(20));
    let final_products = FinalProduct::from_products(
        products,
        &final_categories,
        category_product,
        file_product,
        files,
    )
    .await;

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

    Ok(QueryAllResult {
        product_types: final_product_types,
        products: final_products,
        categories: final_categories,
    })
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct YamlCategories {
    pub meno_typu: String,
    pub meno: String,
    id: u32,
    pub podkategorie: Vec<Self>,
}

enum Maybe<T> {
    Some(T),
    None,
    UseParent,
}

impl YamlCategories {
    pub fn load() -> Self {
        let data =
            //INFO: MAGIC NUMBER!
            std::fs::read_to_string("./filled_out_kategorie.yaml").expect("Unable to read file");
        serde_yaml::from_str(&data).unwrap()
    }

    fn search(&self, category_id: u32) -> Maybe<String> {
        match self.id == category_id {
            true => {
                if self.meno_typu.is_empty() {
                    return Maybe::UseParent;
                }
                Maybe::Some(self.meno_typu.clone())
            }
            false => {
                for podcat in &self.podkategorie {
                    match podcat.search(category_id) {
                        Maybe::Some(prod_type) => return Maybe::Some(prod_type),
                        Maybe::UseParent => {
                            if self.meno_typu.is_empty() {
                                return Maybe::UseParent;
                            }
                            return Maybe::Some(self.meno_typu.clone());
                        }
                        Maybe::None => continue,
                    }
                }
                Maybe::None
            }
        }
    }
    pub fn find_product_type(
        &self,
        category_id: u32,
        product_types: &mut Vec<Rc<RefCell<FinalProductType>>>,
    ) -> Option<Rc<RefCell<FinalProductType>>> {
        let type_name = match self.search(category_id) {
            Maybe::Some(r) => Some(r),
            _ => None,
        };
        if let Some(type_name) = type_name {
            if let Some(product_type) = product_types.iter().find(|t| t.borrow().name == type_name)
            {
                return Some(product_type.clone());
            }
            product_types.push(Rc::new(RefCell::new(FinalProductType {
                name: type_name,
                saleor_id: None,
            })))
        }
        None
    }
}

#[derive(Clone)]
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
    pub async fn from_products(
        products: Vec<Product>,
        categories: &Vec<Rc<RefCell<FinalCategory>>>,
        rel_category_product: Vec<CategoryProduct>,
        file_products: Vec<FileProduct>,
        files: Vec<File>,
    ) -> Vec<Self> {
        //INFO: MAGIC NUMBER!
        let slugify = regex::Regex::new(r###"[^a-zA-Z0-9-]+"###).unwrap();

        //filter out products or test names or deleted while mapping
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(20)
            .connect(gql_queries::SQL_Endpoint)
            .await
            .expect("Failed creating sql connection");

        let mut final_products: Vec<FinalProduct> = products
            .into_iter()
            .filter_map(|product| {
                //INFO: MAGIC NUMBER!
                if product.name.contains("test") || product.deleted_at.is_some() {
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

        for p in final_products.iter_mut() {
            if p.product.name.is_empty() {
                //INFO: MAGIC NUMBER!
                let product_texts_q = sqlx::query_as!(
                    ProductsTexts,
                    "SELECT * FROM products_texts WHERE product_id = ?;",
                    p.product.id
                )
                .fetch_all(&pool);

                let mut product_texts = product_texts_q
                    .await
                    .expect("failed fetching product_texts");
                product_texts.sort_unstable_by(|t, n| n.updated_at.cmp(&t.updated_at));
                p.product.name = product_texts[0].name.clone();
            }
        }

        //Try to find a category product belongs to(with biggest id = newest), then get pointer to it
        for product in final_products.iter_mut() {
            let mut category_matches: Vec<_> = rel_category_product
                .iter()
                .filter_map(|rel| {
                    if rel.product_id == product.product.id {
                        return Some(rel);
                    }
                    None
                })
                .collect();
            category_matches.sort_unstable_by(|a, b| b.id.cmp(&a.id));

            if let Some(category) = category_matches.get(0) {
                product.category = categories
                    .iter()
                    .find(|&cat| cat.borrow().category.borrow().id == category.category_id)
                    .cloned();
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
                    if file_match.mime_type.contains("pdf") {
                        continue;
                    }
                    /*
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
                    */
                    let ip_address = local_ip_address::local_ip()
                        .expect("Failed finding local IP. Are you offline?");
                    let file_name =
            //INFO: MAGIC NUMBER!
                        format!("http://{}:38008/products/{}", ip_address, file_match.name);
                    match_product.images.push(file_name);
                }
            }
        }

        // Avoid clashing SKUs by creating a new norm, aka appending 3 more numbers
        let mut skus: Vec<SKU> = Vec::new();
        for product in final_products.iter_mut() {
            if let Some(matching_sku) = skus.iter_mut().find(|s| s.base_sku == product.SKU) {
                matching_sku.latest_sku_suffix += 1;
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

pub struct CategoryTexts {
    id: i32,
    name: String,
    description: String,
    language_id: u32,
    category_id: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

pub struct ProductsTexts {
    id: u32,
    name: String,
    short_description: String,
    description: String,
    language_id: u32,
    product_id: u32,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
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
    pub image: Option<String>,
    pub product_type: Option<Rc<RefCell<FinalProductType>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FinalProductType {
    pub name: String,
    pub saleor_id: Option<cynic::Id>,
}

impl FinalCategory {
    pub fn new(category: Category) -> Rc<RefCell<Self>> {
        Rc::new_cyclic(|me| {
            RefCell::new(FinalCategory {
                me: me.clone(),
                category: Rc::new(RefCell::new(category)),
                parent_category: None,
                saleor_id: None,
                image: None,
                product_type: None,
            })
        })
    }
    pub fn new_self_rc(&self) -> Rc<RefCell<Self>> {
        self.me
            .upgrade()
            .context("Failed to upgrade pointer to FinalCategory")
            .unwrap()
    }

    pub async fn from_categories(
        categories: Vec<Category>,
        files: Vec<File>,
        product_types: &mut Vec<Rc<RefCell<FinalProductType>>>,
    ) -> Vec<Rc<RefCell<Self>>> {
        //filter out not our categorien
        //CHECK FOR DATE OF DELTETION: IF ANY, FILTER IT OUT
        let mut final_categories: Vec<Rc<RefCell<FinalCategory>>> = categories
            .into_iter()
            .filter_map(|cat| {
                if cat.name.to_lowercase().contains("test") || cat.deleted_at.is_some() {
                    return None;
                }
                //INFO: MAGIC NUMBER!
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

        //fetch missing names and descriptions
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(20)
            .connect(gql_queries::SQL_Endpoint)
            .await
            .expect("Failed creating sql connection");

        for c in final_categories.iter_mut() {
            let cat = c.borrow_mut();
            let mut cat_cat = cat.category.borrow_mut();

            let id = &cat_cat.id.clone();
            let category_texts_q = sqlx::query_as!(
                //INFO: MAGIC NUMBER!
                CategoryTexts,
                "SELECT * FROM categories_texts WHERE category_id = ?;",
                id
            )
            .fetch_all(&pool);

            let mut cat_texts = category_texts_q
                .await
                .expect("failed fetching category_texts");
            if let Some(text) = cat_texts.get(0) {
                cat_cat.description = text.description.clone();
            }
            if cat_cat.name.is_empty() {
                cat_texts.sort_unstable_by(|t, n| n.updated_at.cmp(&t.updated_at));
                if let Some(name) = cat_texts.get(0) {
                    cat_cat.name = name.name.clone()
                };
            }
        }

        //filter out empty named categories(prolly duds)
        let mut final_categories: Vec<Rc<RefCell<FinalCategory>>> = final_categories
            .into_iter()
            .filter(|c| !c.borrow().category.borrow().name.is_empty())
            .collect();

        // dbg!(final_categories[3].borrow().category.borrow());

        //pair categories to sub categories etc
        for category in final_categories.iter_mut() {
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

            //assigns image filename
            let mut cat = category.borrow_mut();
            let mut image: Option<&File> = None;
            {
                let mut cat_cat = cat.category.borrow_mut();
                cat_cat.description =
                    gql_queries::Jsonstring::parse_old_json(&cat_cat.description).to_string();

                if let Some(img_id) = &cat_cat.image_id {
                    image = files.iter().find(|f| f.id == *img_id);
                }
            }
            if let Some(file) = image {
                cat.image = Some(file.name.clone());
            }
        }

        let yaml_cat = YamlCategories::load();
        // find product_type of category
        // Make an array of product_types, compare if new one is actually new,
        // either append new or replace with existing or smt...
        for cat in final_categories.iter_mut() {
            let mut cat = cat.borrow_mut();
            let id = cat.category.borrow().id;
            cat.product_type = yaml_cat.find_product_type(id, product_types);
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
