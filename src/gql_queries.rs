#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

use std::{cell::RefCell, rc::Rc, str::FromStr};

use cynic::{http::SurfExt, GraphQlResponse, Id, MutationBuilder};
use dotenvy_macro::dotenv;
use rust_decimal::Decimal;

use crate::{
    get_sqls::{FinalProduct, FinalProductType, OldJson},
    saleor_login,
};
use serde::de::IgnoredAny;
use surf::Client;

use self::schema::__fields::ProductMedia;

pub const GQL_Endpoint: &str = dotenv!("GRAPHQL_URL");
pub const SQL_Endpoint: &str = dotenv!("DATABASE_URL");
// All producst are in this app assigned to a single warehouse and tax class and channel
pub const Product_Channel_ID: &str = "";
pub const Porudct_Tax_Class_ID: &str = "";
pub const Product_Warehouse_ID: &str = "";

#[cynic::schema("saleor")]
mod schema {}

/*
    ----------------- CREATE TOKEN -------------------
*/
#[derive(cynic::QueryVariables, Debug)]
pub struct CreateTokenVariables<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateTokenVariables")]
pub struct CreateToken {
    #[arguments(email: $email, password: $password)]
    pub token_create: Option<CreateToken2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "CreateToken")]
pub struct CreateToken2 {
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    pub errors: Vec<AccountError>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct AccountError {
    pub field: Option<String>,
    pub message: Option<String>,
}

/*
    ----------------- CREATE CATEGORY -------------------
*/

#[derive(cynic::QueryVariables, Debug)]
pub struct CreateCategoryVariables<'a> {
    pub input: CategoryInput<'a>,
    pub parent_id: Option<&'a cynic::Id>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateCategoryVariables")]
pub struct CreateCategory {
    #[arguments(input: $input, parent: $parent_id)]
    pub category_create: Option<CategoryCreate>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct CategoryCreate {
    pub errors: Vec<ProductError>,
    pub category: Option<Category>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductError {
    pub code: ProductErrorCode,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Category {
    pub id: cynic::Id,
}

#[derive(cynic::Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProductErrorCode {
    AlreadyExists,
    AttributeAlreadyAssigned,
    AttributeCannotBeAssigned,
    AttributeVariantsDisabled,
    MediaAlreadyAssigned,
    DuplicatedInputItem,
    GraphqlError,
    Invalid,
    InvalidPrice,
    ProductWithoutCategory,
    NotProductsImage,
    NotProductsVariant,
    NotFound,
    Required,
    Unique,
    VariantNoDigitalContent,
    CannotManageProductWithoutVariant,
    ProductNotAssignedToChannel,
    UnsupportedMediaProvider,
    PreorderVariantCannotBeDeactivated,
}

#[derive(cynic::InputObject, Debug)]
pub struct CategoryInput<'a> {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<Jsonstring>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub slug: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub seo: Option<SeoInput<'a>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub background_image: Option<Upload>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub background_image_alt: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub private_metadata: Option<Vec<MetadataInput<'a>>>,
}

#[derive(cynic::InputObject, Debug)]
pub struct MetadataInput<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

#[derive(cynic::InputObject, Debug)]
pub struct SeoInput<'a> {
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
}
#[derive(cynic::Scalar, Debug, Clone)]
pub struct Upload(pub String);

#[derive(cynic::Scalar, Debug, Clone)]
#[cynic(graphql_type = "JSONString")]
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

/*
    ----------------- CREATE PRODUDCT TYPES ------------
*/

#[derive(cynic::QueryVariables, Debug)]
pub struct CreateProductTypeVariables<'a> {
    pub input: ProductTypeInput<'a>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "CreateProductTypeVariables")]
pub struct CreateProductType {
    #[arguments(input: $input)]
    pub product_type_create: Option<ProductTypeCreate>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductTypeCreate {
    pub errors: Vec<ProductError>,
    pub product_type: Option<ProductType>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductType {
    pub id: cynic::Id,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum ProductTypeKindEnum {
    Normal,
    GiftCard,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductTypeInput<'a> {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub slug: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProductTypeKindEnum>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub has_variants: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub product_attributes: Option<Vec<&'a cynic::Id>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub variant_attributes: Option<Vec<&'a cynic::Id>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub is_shipping_required: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub is_digital: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub weight: Option<WeightScalar>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tax_code: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tax_class: Option<&'a cynic::Id>,
}

pub async fn create_product_type(
    typ: Rc<RefCell<FinalProductType>>,
    tax_class_id: &cynic::Id,
    client: &mut Client,
    jwt: &String,
) -> Result<GraphQlResponse<CreateProductType, IgnoredAny>, SaleorGraphqlError> {
    let typ = typ.borrow();
    let slug = &typ.name.to_lowercase().replace(" ", "-");
    let create_product_type_operation = CreateProductType::build(CreateProductTypeVariables {
        input: ProductTypeInput {
            is_digital: Some(false),
            tax_class: Some(&tax_class_id),
            tax_code: None,
            slug: Some(slug),
            name: Some(&typ.name),
            kind: Some(ProductTypeKindEnum::Normal),
            has_variants: None,
            weight: Some(WeightScalar("0.5".to_owned())),
            product_attributes: None,
            variant_attributes: None,
            is_shipping_required: Some(true),
        },
    });

    let create_product_type_response = client
        .post(GQL_Endpoint)
        .header("Authorization", jwt)
        .run_graphql(create_product_type_operation)
        .await;

    if let Ok(create_product_type_response) = create_product_type_response {
        let create_product_type_operation = CreateProductType::build(CreateProductTypeVariables {
            input: ProductTypeInput {
                is_digital: Some(false),
                tax_class: Some(&tax_class_id),
                tax_code: None,
                slug: Some(&slug),
                name: Some(&typ.name),
                kind: Some(ProductTypeKindEnum::Normal),
                has_variants: None,
                weight: Some(WeightScalar("0.5".to_owned())),
                product_attributes: None,
                variant_attributes: None,
                is_shipping_required: Some(true),
            },
        });

        if create_product_type_response.errors.is_some()
            || create_product_type_response.data.as_ref().is_some_and(|x| {
                x.product_type_create
                    .as_ref()
                    .is_some_and(|y| y.errors.len() > 0)
            })
        {
            println!("{:?}", &create_product_type_operation.query);
            println!("{:?}", &create_product_type_operation.variables);
            println!("{:?}", &create_product_type_response);
        }

        if let Some(data) = &create_product_type_response.data {
            if let Some(create) = &data.product_type_create {
                for err in &create.errors {
                    println!("{:?}", err);
                    return Err(SaleorGraphqlError::Other(err.code));
                }
            }
        }

        if let Some(data) = &create_product_type_response.errors {
            for dat in data {
                if dat.message == "Signature has expired" {
                    return Err(SaleorGraphqlError::SignatureExpired);
                }
                println!("dat");
                return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
            }
        }
        return Ok(create_product_type_response);
    }
    return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
}

/*
    ----------------- CREATE PRODUCT -------------------
*/
#[derive(cynic::QueryVariables, Debug)]
pub struct VariantCreateVariables<'a> {
    pub input: ProductVariantCreateInput<'a>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ProductCreateVariables<'a> {
    pub input: ProductCreateInput<'a>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct ProductChannelListingUpdateVariables<'a> {
    pub id: &'a cynic::Id,
    pub input: ProductChannelListingUpdateInput<'a>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct VariantChannelListingUpdateVariables<'a> {
    pub id: &'a cynic::Id,
    pub input: Vec<ProductVariantChannelListingAddInput<'a>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "VariantCreateVariables")]
pub struct VariantCreate {
    #[arguments(input: $input)]
    pub product_variant_create: Option<ProductVariantCreate>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductVariantCreate {
    pub product_variant: Option<ProductVariant>,
    pub errors: Vec<ProductError>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "Mutation",
    variables = "VariantChannelListingUpdateVariables"
)]
pub struct VariantChannelListingUpdate {
    #[arguments(id: $id, input: $input)]
    pub product_variant_channel_listing_update: Option<ProductVariantChannelListingUpdate>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductVariantChannelListingUpdate {
    pub variant: Option<ProductVariant>,
    pub errors: Vec<ProductChannelListingError>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductVariant {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ProductCreateVariables")]
pub struct ProductCreate {
    #[arguments(input: $input)]
    pub product_create: Option<ProductCreate2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ProductCreate")]
pub struct ProductCreate2 {
    pub product: Option<Product>,
    pub errors: Vec<ProductError>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Product {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    graphql_type = "Mutation",
    variables = "ProductChannelListingUpdateVariables"
)]
pub struct ProductChannelListingUpdate {
    #[arguments(id: $id, input: $input)]
    pub product_channel_listing_update: Option<ProductChannelListingUpdate2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ProductChannelListingUpdate")]
pub struct ProductChannelListingUpdate2 {
    pub errors: Vec<ProductChannelListingError>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct ProductChannelListingError {
    pub field: Option<String>,
    pub message: Option<String>,
    pub code: ProductErrorCode,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductVariantCreateInput<'a> {
    pub attributes: Vec<AttributeValueInput<'a>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sku: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub track_inventory: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub weight: Option<WeightScalar>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub preorder: Option<PreorderSettingsInput>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub quantity_limit_per_customer: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub private_metadata: Option<Vec<MetadataInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub external_reference: Option<&'a str>,
    pub product: &'a cynic::Id,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub stocks: Option<Vec<StockInput<'a>>>,
}

#[derive(cynic::InputObject, Debug)]
pub struct StockInput<'a> {
    pub warehouse: &'a cynic::Id,
    pub quantity: i32,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductVariantChannelListingAddInput<'a> {
    pub channel_id: &'a cynic::Id,
    pub price: PositiveDecimal,
    pub cost_price: Option<PositiveDecimal>,
    pub preorder_threshold: Option<i32>,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductCreateInput<'a> {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<AttributeValueInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub category: Option<&'a cynic::Id>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub charge_taxes: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub collections: Option<Vec<&'a cynic::Id>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<Jsonstring>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub slug: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tax_class: Option<&'a cynic::Id>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub tax_code: Option<&'a str>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub seo: Option<SeoInput<'a>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub weight: Option<WeightScalar>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Vec<MetadataInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub private_metadata: Option<Vec<MetadataInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub external_reference: Option<&'a str>,
    pub product_type: &'a cynic::Id,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductChannelListingUpdateInput<'a> {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub update_channels: Option<Vec<ProductChannelListingAddInput<'a>>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub remove_channels: Option<Vec<&'a cynic::Id>>,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductChannelListingAddInput<'a> {
    pub channel_id: &'a cynic::Id,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub is_published: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<Date>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub visible_in_listings: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub is_available_for_purchase: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub available_for_purchase_date: Option<Date>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub available_for_purchase_at: Option<DateTime>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub add_variants: Option<Vec<&'a cynic::Id>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub remove_variants: Option<Vec<&'a cynic::Id>>,
}

#[derive(cynic::InputObject, Debug)]
pub struct PreorderSettingsInput {
    pub global_threshold: Option<i32>,
    pub end_date: Option<DateTime>,
}

#[derive(cynic::InputObject, Debug)]
pub struct AttributeValueInput<'a> {
    pub id: Option<&'a cynic::Id>,
    pub external_reference: Option<&'a str>,
    pub values: Option<Vec<&'a str>>,
    pub dropdown: Option<AttributeValueSelectableTypeInput<'a>>,
    pub swatch: Option<AttributeValueSelectableTypeInput<'a>>,
    pub multiselect: Option<Vec<AttributeValueSelectableTypeInput<'a>>>,
    pub numeric: Option<&'a str>,
    pub file: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub references: Option<Vec<&'a cynic::Id>>,
    pub rich_text: Option<Jsonstring>,
    pub plain_text: Option<&'a str>,
    pub boolean: Option<bool>,
    pub date: Option<Date>,
    pub date_time: Option<DateTime>,
}

#[derive(cynic::InputObject, Debug)]
pub struct AttributeValueSelectableTypeInput<'a> {
    pub id: Option<&'a cynic::Id>,
    pub external_reference: Option<&'a str>,
    pub value: Option<&'a str>,
}

#[derive(cynic::Scalar, Debug, Clone)]
pub struct Date(pub String);

#[derive(cynic::Scalar, Debug, Clone)]
pub struct DateTime(pub String);

#[derive(cynic::Scalar, Debug, Clone)]
pub struct PositiveDecimal(pub Decimal);

#[derive(cynic::Scalar, Debug, Clone)]
pub struct WeightScalar(pub String);

#[derive(cynic::QueryVariables, Debug)]
pub struct ProductMediaCreateVariables<'a> {
    pub input: ProductMediaCreateInput<'a>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "ProductMediaCreateVariables")]
pub struct ProductMediaCreate {
    #[arguments(input: $input)]
    pub product_media_create: Option<ProductMediaCreate2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ProductMediaCreate")]
pub struct ProductMediaCreate2 {
    pub errors: Vec<ProductError>,
    pub media: Option<ProductMedia2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "ProductMedia")]
pub struct ProductMedia2 {
    pub id: cynic::Id,
}

#[derive(cynic::InputObject, Debug)]
pub struct ProductMediaCreateInput<'a> {
    pub alt: Option<&'a str>,
    pub image: Option<Upload>,
    pub product: &'a cynic::Id,
    pub media_url: Option<String>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum SaleorGraphqlError {
    SignatureExpired,
    Other(ProductErrorCode),
}

/* --- ASSING MEDIA TO VARIANTS --- */

#[derive(cynic::QueryVariables, Debug)]
pub struct VariantMediaAssignVariables<'a> {
    pub media_id: &'a cynic::Id,
    pub variant_id: &'a cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Mutation", variables = "VariantMediaAssignVariables")]
pub struct VariantMediaAssign {
    #[arguments(mediaId: $media_id, variantId: $variant_id)]
    pub variant_media_assign: Option<VariantMediaAssign2>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "VariantMediaAssign")]
pub struct VariantMediaAssign2 {
    pub errors: Vec<ProductError>,
}

/* --- ACTUAL QUERIES--- */

pub async fn create_product(
    category_id: Option<&Id>,
    product: &mut FinalProduct,
    tax_class_id: &Id,
    default_product_type_id: &Id,
    old_id: u32,
    client: &mut Client,
    jwt: &mut String,
) -> Result<GraphQlResponse<ProductCreate, IgnoredAny>, SaleorGraphqlError> {
    //If product type has saleor_id, use that, else create product type and assign that, else use
    //default product type id
    let mut product_type_id: cynic::Id = default_product_type_id.clone();
    if let Some(category) = &product.category {
        if let Some(product_type) = &category.borrow().product_type {
            let saleor_id = product_type.borrow().saleor_id.clone();
            if let Some(prod_type_saleor_id) = saleor_id {
                product_type_id = prod_type_saleor_id.clone();
            } else {
                println!("creating product type {}", &product_type.borrow().name);
                'a: loop {
                    let create_product_type_result =
                        create_product_type(product_type.clone(), tax_class_id, client, jwt).await;
                    match create_product_type_result {
                        Err(e) => match e {
                            SaleorGraphqlError::Other(ee) => {
                                println!(
                                    "create product type '{}' failed, code: {:?}",
                                    product_type.borrow().name,
                                    ee
                                );
                                break 'a;
                            }
                            SaleorGraphqlError::SignatureExpired => {
                                let (new_client, new_jwt) = saleor_login()
                                    .await
                                    .expect("failed to create product type during product cuz prolly signature");
                                *jwt = new_jwt;
                                *client = new_client;
                            }
                        },
                        Ok(data) => {
                            if let Some(data) = data.data {
                                if let Some(product_type_create) = data.product_type_create {
                                    if let Some(prd) = product_type_create.product_type {
                                        product_type.borrow_mut().saleor_id = Some(prd.id.clone());
                                        product_type_id = prd.id;
                                        println!("success!");
                                    }
                                }
                            }
                            break 'a;
                        }
                    }
                }
            }
        }
    }
    let description = Jsonstring::from_string(product.product.description.clone());
    let weight = product.product.weight.map(|w| WeightScalar(w.to_string()));
    let old_id = old_id.to_string();
    let create_product_operation = ProductCreate::build(ProductCreateVariables {
        input: ProductCreateInput {
            attributes: None,
            category: category_id,
            charge_taxes: Some(true),
            collections: None,
            description: Some(description.clone()),
            name: Some(product.product.name.as_str()),
            slug: Some(product.slug.as_str()),
            tax_class: Some(tax_class_id),
            tax_code: None,
            seo: None,
            weight: weight.clone(),
            rating: None,
            metadata: Some(vec![
                MetadataInput {
                    key: "short_description",
                    value: product.product.short_description.as_str(),
                },
                MetadataInput {
                    key: "old_id",
                    value: &old_id,
                },
            ]),
            private_metadata: None,
            external_reference: None,
            product_type: &product_type_id,
        },
    });

    let create_product_response = client
        .post(GQL_Endpoint)
        .header("Authorization", jwt.clone())
        .run_graphql(create_product_operation)
        .await;

    if let Ok(create_product_response) = create_product_response {
        //Wish I didn't have to do this tho
        let create_product_operation = ProductCreate::build(ProductCreateVariables {
            input: ProductCreateInput {
                attributes: None,
                category: category_id,
                charge_taxes: Some(true),
                collections: None,
                description: Some(description),
                name: Some(product.product.name.as_str()),
                slug: Some(product.slug.as_str()),
                tax_class: Some(tax_class_id),
                tax_code: None,
                seo: None,
                weight,
                rating: None,
                metadata: None,
                private_metadata: None,
                external_reference: None,
                product_type: &product_type_id,
            },
        });
        if create_product_response.errors.is_some()
            || create_product_response.data.as_ref().is_some_and(|x| {
                x.product_create
                    .as_ref()
                    .is_some_and(|y| y.errors.len() > 0)
            })
        {
            println!("{:?}", &create_product_operation.query);
            println!("{:?}", &create_product_operation.variables);
            println!("{:?}", &create_product_response);
        }

        if let Some(data) = &create_product_response.data {
            if let Some(create) = &data.product_create {
                for err in &create.errors {
                    println!("{:?}", err);
                    return Err(SaleorGraphqlError::Other(err.code));
                }
            }
        }

        if let Some(data) = &create_product_response.errors {
            for dat in data {
                if dat.message == "Signature has expired" {
                    return Err(SaleorGraphqlError::SignatureExpired);
                }
                println!("dat");
                return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
            }
        }
        return Ok(create_product_response);
    }
    return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
}

pub async fn product_channel_listing_update(
    product: &mut FinalProduct,
    client: &mut Client,
    jwt: &String,
    channel_id: &Id,
) -> Result<GraphQlResponse<ProductChannelListingUpdate>, SaleorGraphqlError> {
    if let Some(product_saleor_id) = &product.saleor_id {
        //4.1 productChannelListingUpdate
        // product
        //                             .product
        //                             .created_at
        // .map(|d| gql_queries::Date(d.to_rfc3339()))

        let channel_listing_update_operation =
            ProductChannelListingUpdate::build(ProductChannelListingUpdateVariables {
                id: product_saleor_id,
                input: ProductChannelListingUpdateInput {
                    update_channels: Some(vec![ProductChannelListingAddInput {
                        add_variants: None,
                        remove_variants: None,
                        available_for_purchase_at: None,
                        published_at: None,
                        available_for_purchase_date: None,
                        publication_date: None,
                        channel_id,
                        is_available_for_purchase: Some(true),
                        is_published: Some(true),
                        visible_in_listings: Some(true),
                    }]),
                    remove_channels: None,
                },
            });

        let channel_listing_update_response = client
            .post(GQL_Endpoint)
            .header("Authorization", jwt)
            .run_graphql(channel_listing_update_operation)
            .await;
        if let Ok(channel_listing_update_response) = channel_listing_update_response {
            let channel_listing_update_operation =
                ProductChannelListingUpdate::build(ProductChannelListingUpdateVariables {
                    id: product_saleor_id,
                    input: ProductChannelListingUpdateInput {
                        update_channels: Some(vec![ProductChannelListingAddInput {
                            add_variants: None,
                            remove_variants: None,
                            available_for_purchase_at: None,
                            published_at: None,
                            available_for_purchase_date: None,
                            publication_date: None,
                            channel_id,
                            is_available_for_purchase: Some(true),
                            is_published: Some(true),
                            visible_in_listings: Some(true),
                        }]),
                        remove_channels: None,
                    },
                });

            if channel_listing_update_response.errors.is_some()
                || channel_listing_update_response
                    .data
                    .as_ref()
                    .is_some_and(|x| {
                        x.product_channel_listing_update
                            .as_ref()
                            .is_some_and(|y| y.errors.len() > 0)
                    })
            {
                println!("{:?}", &channel_listing_update_operation.query);
                println!("{:?}", &channel_listing_update_operation.variables);
                println!("{:?}", &channel_listing_update_response);
            }
            if let Some(data) = &channel_listing_update_response.data {
                if let Some(data) = &data.product_channel_listing_update {
                    for err in &data.errors {
                        println!("{:?}", err);
                        return Err(SaleorGraphqlError::Other(err.code));
                    }
                }
            }
            if let Some(data) = &channel_listing_update_response.errors {
                for dat in data {
                    if dat.message == "Signature has expired" {
                        return Err(SaleorGraphqlError::SignatureExpired);
                    }
                    println!("dat");
                    return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
                }
            }
            Ok(channel_listing_update_response)
        } else {
            Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError))
        }
    } else {
        Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError))
    }
}

pub async fn variant_create(
    product: &mut FinalProduct,
    client: &mut Client,
    jwt: &String,
    warehouse_id: &Id,
) -> Result<GraphQlResponse<VariantCreate>, SaleorGraphqlError> {
    if let Some(saleor_product_id) = &product.saleor_id {
        let mut stocks: Option<Vec<StockInput>> = None;
        if let Some(q) = product.product.quantity {
            stocks = Some(vec![StockInput {
                warehouse: warehouse_id,
                quantity: q,
            }]);
        };
        let variant_create_operation = VariantCreate::build(VariantCreateVariables {
            input: ProductVariantCreateInput {
                product: saleor_product_id,
                sku: Some(&product.SKU),
                external_reference: None,
                name: None,
                attributes: Vec::new(),
                metadata: None,
                private_metadata: None,
                preorder: None,
                quantity_limit_per_customer: None,
                stocks,
                track_inventory: Some(true),
                weight: None,
            },
        });

        let variant_create_response = client
            .post(GQL_Endpoint)
            .header("Authorization", jwt)
            .run_graphql(variant_create_operation)
            .await;

        if let Ok(variant_create_response) = variant_create_response {
            let variant_create_operation = VariantCreate::build(VariantCreateVariables {
                input: ProductVariantCreateInput {
                    product: saleor_product_id,
                    sku: Some(product.product.code.as_str()),
                    external_reference: None,
                    name: None,
                    attributes: Vec::new(),
                    metadata: None,
                    private_metadata: None,
                    preorder: None,
                    quantity_limit_per_customer: None,
                    stocks: None,
                    track_inventory: Some(true),
                    weight: None,
                },
            });

            if variant_create_response.errors.is_some()
                || variant_create_response.data.as_ref().is_some_and(|x| {
                    x.product_variant_create
                        .as_ref()
                        .is_some_and(|y| y.errors.len() > 0)
                })
            {
                println!("{:?}", &variant_create_operation.query);
                println!("{:?}", &variant_create_operation.variables);
                println!("{:?}", &variant_create_response);
            }

            if let Some(data) = &variant_create_response.data {
                if let Some(data) = &data.product_variant_create {
                    for err in &data.errors {
                        println!("{:?}", err);
                        return Err(SaleorGraphqlError::Other(err.code));
                    }
                }
            }

            if let Some(data) = &variant_create_response.errors {
                for dat in data {
                    if dat.message == "Signature has expired" {
                        return Err(SaleorGraphqlError::SignatureExpired);
                    }
                    println!("dat");
                    return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
                }
            }
            return Ok(variant_create_response);
        }
        return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
    }
    Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError))
}

pub async fn variant_listing_update(
    product: &mut FinalProduct,
    variant_id: &Id,
    channel_id: &Id,
    client: &mut Client,
    jwt: &String,
) -> Result<GraphQlResponse<VariantChannelListingUpdate>, SaleorGraphqlError> {
    if let Some(price) = product.price.as_ref().map(|price| {
        PositiveDecimal(
            Decimal::from_str(&price)
                .unwrap_or(Decimal::new(0, 2))
                .round_dp(2),
        )
    }) {
        let variant_listing_update_operation =
            VariantChannelListingUpdate::build(VariantChannelListingUpdateVariables {
                id: variant_id,
                input: vec![ProductVariantChannelListingAddInput {
                    channel_id,
                    cost_price: None,
                    preorder_threshold: None,
                    price: price.clone(),
                }],
            });

        let variant_listing_update_response = client
            .post(GQL_Endpoint)
            .header("Authorization", jwt)
            .run_graphql(variant_listing_update_operation)
            .await;
        if let Ok(variant_listing_update_response) = variant_listing_update_response {
            let variant_listing_update_operation =
                VariantChannelListingUpdate::build(VariantChannelListingUpdateVariables {
                    id: variant_id,
                    input: vec![ProductVariantChannelListingAddInput {
                        channel_id,
                        cost_price: None,
                        preorder_threshold: None,
                        price,
                    }],
                });

            if variant_listing_update_response.errors.is_some()
                || variant_listing_update_response
                    .data
                    .as_ref()
                    .is_some_and(|x| {
                        x.product_variant_channel_listing_update
                            .as_ref()
                            .is_some_and(|y| y.errors.len() > 0)
                    })
            {
                println!("{:?}", &variant_listing_update_operation.query);
                println!("{:?}", &variant_listing_update_operation.variables);
                println!("{:?}", &variant_listing_update_response);
            }

            if let Some(data) = &variant_listing_update_response.errors {
                for dat in data {
                    if dat.message == "Signature has expired" {
                        return Err(SaleorGraphqlError::SignatureExpired);
                    }
                    println!("dat");
                    return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
                }
            }
            return Ok(variant_listing_update_response);
        }
        return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
    }
    Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError))
}

pub async fn product_media_create(
    product: &mut FinalProduct,
    client: &mut Client,
    jwt: &mut String,
) -> Result<Vec<GraphQlResponse<ProductMediaCreate>>, SaleorGraphqlError> {
    if let Some(saleor_product_id) = &product.saleor_id {
        let mut media_create_operations = vec![];
        for image in product.images.iter() {
            media_create_operations.push(ProductMediaCreate::build(ProductMediaCreateVariables {
                input: ProductMediaCreateInput {
                    alt: Some(product.product.name.as_str()),
                    image: None,
                    product: saleor_product_id,
                    media_url: Some(image.clone()),
                },
            }));
        }
        let mut responses = vec![];
        for media_create_operation in media_create_operations {
            let media_create_response = client
                .post(GQL_Endpoint)
                .header("Authorization", &*jwt)
                .run_graphql(media_create_operation)
                .await;
            if let Ok(media_create_response) = media_create_response {
                if media_create_response.errors.is_some()
                    || media_create_response.data.as_ref().is_some_and(|x| {
                        x.product_media_create
                            .as_ref()
                            .is_some_and(|y| y.errors.len() > 0)
                    })
                {
                    println!("{:?}", &media_create_response);
                }

                if let Some(data) = &media_create_response.errors {
                    for dat in data {
                        if dat.message == "Signature has expired" {
                            if let Ok((new_client, new_jwt)) = saleor_login().await {
                                *jwt = new_jwt;
                                *client = new_client;
                            } else {
                                return Err(SaleorGraphqlError::Other(ProductErrorCode::Invalid));
                            }
                        } else {
                            return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
                        }
                    }
                }
                responses.push(media_create_response);
            }
        }
        Ok(responses)
    } else {
        Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError))
    }
}

pub async fn variant_media_assign(
    variant_id: &Id,
    media_id: &Id,
    client: &mut Client,
    jwt: &String,
) -> Result<GraphQlResponse<VariantMediaAssign>, SaleorGraphqlError> {
    let variant_media_assign_operation = VariantMediaAssign::build(VariantMediaAssignVariables {
        variant_id,
        media_id,
    });

    let variant_media_assign_result = client
        .post(GQL_Endpoint)
        .header("Authorization", jwt)
        .run_graphql(variant_media_assign_operation)
        .await;

    if let Ok(variant_media_assign_result) = variant_media_assign_result {
        let variant_media_assign_operation =
            VariantMediaAssign::build(VariantMediaAssignVariables {
                variant_id,
                media_id,
            });

        if variant_media_assign_result.errors.is_some()
            || variant_media_assign_result.data.as_ref().is_some_and(|x| {
                x.variant_media_assign
                    .as_ref()
                    .is_some_and(|y| y.errors.len() > 0)
            })
        {
            println!("{:?}", &variant_media_assign_operation.query);
            println!("{:?}", &variant_media_assign_operation.variables);
            println!("{:?}", &variant_media_assign_operation);
        }

        if let Some(data) = &variant_media_assign_result.errors {
            for dat in data {
                if dat.message == "Signature has expired" {
                    return Err(SaleorGraphqlError::SignatureExpired);
                }
                println!("dat");
                return Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError));
            }
        }
        return Ok(variant_media_assign_result);
    }
    Err(SaleorGraphqlError::Other(ProductErrorCode::GraphqlError))
}
