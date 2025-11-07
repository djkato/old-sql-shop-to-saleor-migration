#![allow(non_snake_case)]

mod get_sqls;
mod gql_queries;
mod tests;

use anyhow::Context;
use cynic::MutationBuilder;
use cynic::{http::SurfExt, GraphQlResponse};
use dotenvy_macro::dotenv;
use get_sqls::query_all;
use gql_queries::{product_channel_listing_update, CreateTokenVariables, GQL_Endpoint};
use reqwest::multipart::{Form, Part};

use std::io::prelude::*;
use std::rc::Rc;
use std::{cell::RefCell, fs::File};

use crate::get_sqls::FinalProductType;
use crate::gql_queries::{
    create_product, create_product_type, product_media_create, variant_create,
    variant_listing_update, variant_media_assign, CreateCategory, MetadataInput,
    Porudct_Tax_Class_ID, ProductErrorCode, Product_Channel_ID, Product_Warehouse_ID,
    SaleorGraphqlError,
};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //std::env::set_var("RUST_BACKTRACE", "1");
    /*jä
    use tokio::process::Command;
    Command::new("npm")
        .args(["run", "start"])
        //might wanna replace this if not running on my mashine lol
        .current_dir(r"/home/djkato/code/db-migration/media")
        .kill_on_drop(true)
        .spawn()
        .expect("Failed to serve media");
    println!("Started media serve process");
    //Start the media server so it can serve images
    //   tokio::spawn(async move {
    */

    // });
    let dec = rust_decimal::Decimal::new(200, 2);
    // To write errors to
    let mut log_file = std::fs::OpenOptions::new()
        .append(true)
        .write(true)
        .create(true)
        .open("errors.log")?;

    println!("Querying all data from Old db...");
    let data = query_all().await?;
    let categories = data.categories;
    let mut products = data.products;
    let _product_types = data.product_types;
    println!("Success!");
    let (mut client, mut jwt) = saleor_login().await?;
    //INFO: MAGIC NUMBER!
    let pure_jwt = jwt.split_at(7).1;
    let tax_class_id = cynic::Id::new(gql_queries::Porudct_Tax_Class_ID);
    /*
    Strategy:
    keep on checking if signature has expired, if so re-fresh signature and continue
    ~~1. [x] Iterate the folder with media, category images have format category_{id}.{format}
    MAKE SURE THE FOLDER POINTS TO CORRECT FOLDER. "HEAD 404 IN 1MS HEAD /{img}.jpg" instead /{folder}/{img}.jpg
    & match to corresponding image name ID to category ID, put path to it in inside FinalCategory.~~
    2. [x] start creating the categories from root going deeper inwards
    3. [] query for the categories with SQL and add the image urls directly
    4. [x] start creating the products, linking them to categories
    5. [x] Turn on an html server that can serve the images, use ProductMediaInputs MediaURL!!!
    */

    //    -----CREATING DEFAULT PRODUCT TYPE -----
    let default_product_type = create_product_type(
        Rc::new(RefCell::new(FinalProductType {
            saleor_id: None,
            //INFO: MAGIC NUMBER!
            name: "Základný typ".to_owned(),
        })),
        &tax_class_id,
        &mut client,
        &jwt,
    )
    .await
    .unwrap()
    .data
    .unwrap()
    .product_type_create
    .unwrap()
    .product_type
    .unwrap()
    .id;
    //    -----UPLOADING CATEGORIES-----
    //2.
    //Categories that don't have a parent_id are guaranteed to be at root, so when the root ones are created
    //I assign it's new Saleor ID to the root ones, and next time someone needs to parent under it with ID it'll be there
    let mut cat_slugs = vec![];
    let mut cert_buf: Vec<u8> = vec![];
    // File::open("root.crt")?.read_to_end(&mut cert_buf)?;
    // let cert = reqwest::Certificate::from_pem(&cert_buf)?;

    let reqw_client = reqwest::Client::builder()
        .tls_built_in_root_certs(false)
        .danger_accept_invalid_certs(true)
        // IF PROBLEMS WITH LOCALHOST/ SELF SIGNED CERTS!
        // .add_root_certificate(cert)
        .build()?;

    for category in &categories {
        let mut category_mut = category.borrow_mut();
        {
            let category_data = category_mut.category.clone();
            let mut category_data = category_data.borrow_mut();

            //If slug isn't unique, add random stuff at the end and error log it
            if cat_slugs.contains(&category_data.slug) {
                use rand::distributions::{Alphanumeric, DistString};
                let id = Alphanumeric.sample_string(&mut rand::thread_rng(), 4);
                let slug = category_data.slug.clone();
                category_data.slug = format!("{}-{}", slug, id);
                writeln!(
                    log_file,
                    "category '{}: {}' failed, code: {}",
                    &category_data.id, &category_data.name, "Unique"
                )?;
            }

            cat_slugs.push(category_data.slug.clone());

            println!("Creating category {:?}", &category_data.name);
        }
        //So they can parent eachother as the loop progresses we need to keep the parent_ids alive
        let mut category_parent_id = None;
        let temp_saleor_id;
        if let Some(parent_category) = &mut category_mut.parent_category {
            if let Some(saleor_id) = &parent_category.borrow().saleor_id {
                temp_saleor_id = saleor_id.clone();
                category_parent_id = Some(&temp_saleor_id);
            }
        }
        let category_cp = category_mut.category.clone();
        let category_cp = category_cp.borrow();
        let category_description =
            Some(gql_queries::Jsonstring(category_cp.description.to_owned()));
        let category_old_id = category_cp.id.to_string();

        let category_create_operation: cynic::Operation<
            gql_queries::CreateCategory,
            gql_queries::CreateCategoryVariables,
        > = gql_queries::CreateCategory::build(gql_queries::CreateCategoryVariables {
            input: gql_queries::CategoryInput {
                description: category_description,
                name: Some(category_cp.name.as_str()),
                slug: Some(category_cp.slug.as_str()),
                seo: None,
                background_image: None,
                background_image_alt: Some(category_cp.name.as_str()),
                metadata: Some(vec![MetadataInput {
                    //INFO: MAGIC NUMBER!
                    key: "old_id",
                    value: &category_old_id,
                }]),
                private_metadata: None,
            },
            parent_id: category_parent_id,
        });
        // dbg!(
        //     &category_create_operation.query,
        //     &category_create_operation.variables,
        // );
        let create_cat_response: GraphQlResponse<CreateCategory>;
        // TODO: should be &category_mut.image in prod, but I self signed certs are hell with
        // reqwest

        if let Some(image) = &category_mut.image {
            //INFO: MAGIC NUMBER!
            let file_path_str = format!("/home/djkato/db-migration/media/products/{image}");
            let file_path = std::path::Path::new(&file_path_str);
            if file_path.exists() {
                let mut file_ext = file_path
                    .extension()
                    .context("missing image extension")?
                    .to_str()
                    .context("missing image extension")?;
                //INFO: MAGIC NUMBER!
                if file_ext.to_lowercase() == "jpg" {
                    file_ext = "jpeg"
                }
                let file = std::fs::read(file_path)?;
                let file_part = Part::bytes(file)
                    .file_name(image.clone())
                    .mime_str(format!("image/{file_ext}").as_str())?;
                let map_part = Part::text(
                    serde_json::json!(
                            {
                                "1": [
                                    "variables.input.backgroundImage"
                                ]
                            }
                    )
                    .to_string(),
                );
                let gql_part = Part::text(serde_json::to_string(&category_create_operation)?);

                let form = Form::new()
                    .part("operations", gql_part)
                    .part("map", map_part)
                    .part("1", file_part);
                // And finally, send the form

                let req = reqw_client
                    .post(GQL_Endpoint)
                    .bearer_auth(&pure_jwt)
                    .multipart(form)
                    .send()
                    .await?;

                create_cat_response = serde_json::from_str(&req.text().await?)?;
            } else {
                create_cat_response = surf::post(GQL_Endpoint)
                    .header("Authorization", &jwt)
                    .run_graphql(category_create_operation)
                    .await
                    .expect("Failed creating category");
            }
        } else {
            create_cat_response = surf::post(GQL_Endpoint)
                .header("Authorization", &jwt)
                .run_graphql(category_create_operation)
                .await
                .expect("Failed creating category");
        }
        if let Some(data) = &create_cat_response.data {
            if let Some(create) = &data.category_create {
                for error in &create.errors {
                    dbg!(error);
                }
            }
        }
        if let Some(errors) = &create_cat_response.errors {
            dbg!(&errors);
            dbg!(&create_cat_response);
        }
        if let Some(data) = create_cat_response.data {
            if let Some(create) = data.category_create {
                for err in create.errors {
                    writeln!(
                        log_file,
                        "category '{}: {}' failed, code: {:?}",
                        category_mut.category.borrow().id,
                        category_mut.category.borrow().name,
                        err.code,
                    )?;
                    println!(
                        "category '{}: {}' failed, code: {:?}",
                        category_mut.category.borrow().id,
                        category_mut.category.borrow().name,
                        err.code,
                    );
                }
                if let Some(cat) = create.category {
                    category_mut.saleor_id = Some(cat.id)
                }
            }
        } else {
            category_mut.saleor_id = None;
        }
    }
    //4.
    //Upload products. Check https://www.notion.so/creating-a-product-5e7397a0234d47038aa8a1689d3e61a8
    let warehouse_id = cynic::Id::new(Product_Warehouse_ID);
    let channel_id = cynic::Id::new(Product_Channel_ID);

    for product in &mut products {
        println!("creating product {}", &product.product.name.clone());
        let mut category_id = None;
        let mut cat_temp_id = None;
        if let Some(curr_cat) = &product.category {
            cat_temp_id = curr_cat.borrow().saleor_id.clone();
            category_id = cat_temp_id.as_ref();
        }
        if cat_temp_id.is_none() {
            continue;
        }
        loop {
            let create_product_response = create_product(
                category_id,
                product,
                &tax_class_id,
                &default_product_type,
                product.product.id,
                &mut client,
                &mut jwt,
            )
            .await;

            match create_product_response {
                Err(e) => match e {
                    SaleorGraphqlError::Other(c) => {
                        writeln!(
                            log_file,
                            "create product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        )?;
                        println!(
                            "create product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        );
                        if c == ProductErrorCode::Unique {
                            use rand::distributions::{Alphanumeric, DistString};
                            let id = Alphanumeric.sample_string(&mut rand::thread_rng(), 4);
                            product.slug = format!("{}-{}", product.slug, id);
                        } else {
                            break;
                        }
                    }
                    SaleorGraphqlError::SignatureExpired => (client, jwt) = saleor_login().await?,
                },
                Ok(data) => {
                    if let Some(data) = data.data {
                        if let Some(product_create) = data.product_create {
                            if let Some(prd) = product_create.product {
                                product.saleor_id = Some(prd.id);
                                break;
                            }
                        }
                    }
                    panic!()
                }
            }
        }

        loop {
            let product_channel_listing_update_response =
                product_channel_listing_update(product, &mut client, &jwt, &channel_id).await;
            match product_channel_listing_update_response {
                Err(e) => match e {
                    SaleorGraphqlError::Other(c) => {
                        writeln!(
                            log_file,
                            "product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        )?;
                        println!(
                            "product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        );
                        break;
                    }
                    SaleorGraphqlError::SignatureExpired => (client, jwt) = saleor_login().await?,
                },
                Ok(_) => break,
            }
        }
        let mut variant_id = None;
        loop {
            let variant_create_response =
                variant_create(product, &mut client, &jwt, &warehouse_id).await;
            match variant_create_response {
                Ok(v) => {
                    variant_id = Some(
                        v.data
                            .context("no data variant response")?
                            .product_variant_create
                            .context("no variant create response")?
                            .product_variant
                            .context("no product_variant create response")?
                            .id,
                    );
                    break;
                }
                Err(e) => match e {
                    SaleorGraphqlError::Other(c) => {
                        writeln!(
                            log_file,
                            "product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        )?;
                        println!(
                            "product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        );
                        break;
                    }
                    SaleorGraphqlError::SignatureExpired => (client, jwt) = saleor_login().await?,
                },
            }
        }

        if let Some(variant_id) = &variant_id {
            loop {
                let variant_listing_update_response =
                    variant_listing_update(product, &variant_id, &channel_id, &mut client, &jwt)
                        .await;
                match variant_listing_update_response {
                    Ok(_) => break,
                    Err(e) => match e {
                        SaleorGraphqlError::Other(c) => {
                            writeln!(
                                log_file,
                                "product '{}: {}' failed, code: {:?}",
                                product.product.id, product.product.name, c,
                            )?;
                            println!(
                                "product '{}: {}' failed, code: {:?}",
                                product.product.id, product.product.name, c,
                            );
                            break;
                        }
                        SaleorGraphqlError::SignatureExpired => {
                            (client, jwt) = saleor_login().await?
                        }
                    },
                }
            }
        }

        //5.
        //Upload media for products. https://docs.saleor.io/docs/3.x/api-reference/products/inputs/product-media-create-input#
        let mut media_ids: Vec<cynic::Id> = vec![];
        loop {
            let media_create_responses = product_media_create(product, &mut client, &mut jwt).await;
            match media_create_responses {
                Ok(r) => {
                    media_ids.append(
                        &mut r
                            .into_iter()
                            .filter_map(|d| {
                                d.data.and_then(|o| {
                                    o.product_media_create
                                        .and_then(|ot| ot.media.map(|oto| oto.id))
                                })
                            })
                            .collect::<Vec<cynic::Id>>(),
                    );
                    break;
                }
                Err(e) => match e {
                    SaleorGraphqlError::Other(c) => {
                        writeln!(
                            log_file,
                            "product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        )?;
                        println!(
                            "product '{}: {}' failed, code: {:?}",
                            product.product.id, product.product.name, c,
                        );
                        break;
                    }
                    SaleorGraphqlError::SignatureExpired => (client, jwt) = saleor_login().await?,
                },
            }
        }

        //6.
        // Assing media to the variant we created
        if let Some(variant_id) = variant_id {
            println!("variant_id:{:?}, media_ids: {:?}", &variant_id, &media_ids);
            for media_id in media_ids {
                loop {
                    let variant_media_assign_response =
                        variant_media_assign(&variant_id, &media_id, &mut client, &jwt).await;
                    match variant_media_assign_response {
                        Ok(_) => break,
                        Err(e) => match e {
                            SaleorGraphqlError::Other(c) => {
                                writeln!(
                                    log_file,
                                    "product '{}: {}' failed, code: {:?}",
                                    product.product.id, product.product.name, c,
                                )?;
                                println!(
                                    "product '{}: {}' failed, code: {:?}",
                                    product.product.id, product.product.name, c,
                                );
                                break;
                            }
                            SaleorGraphqlError::SignatureExpired => {
                                (client, jwt) = saleor_login().await?
                            }
                        },
                    }
                }
            }
        }
    }
    anyhow::Ok(())
}

use async_native_tls::{Certificate, TlsConnector};
use std::sync::Arc;
use surf::Client;

async fn saleor_login() -> anyhow::Result<(Client, String)> {
    let tls_connector = Some(Arc::new(
        async_native_tls::TlsConnector::new()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true),
    ));

    let config = surf::Config::new().set_tls_config(tls_connector);
    let client: Client = config.try_into()?;

    println!("Logging into saleor...");
    let login_operation = gql_queries::CreateToken::build(CreateTokenVariables {
        email: dotenv!("EMAIL"),
        password: dotenv!("PASS"),
    });
    dbg!(&GQL_Endpoint);
    let login_response = client
        .post(GQL_Endpoint)
        .run_graphql(login_operation)
        .await
        .unwrap();
    let jwt = format!(
        "Bearer {}",
        login_response
            .data
            .context("")?
            .token_create
            .context("")?
            .token
            .context("")?
    );
    println!("Success!");
    Ok((client, jwt))
}
