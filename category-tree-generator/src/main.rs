#![allow(non_snake_case)]

mod get_sqls;

use get_sqls::query_all;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Querying all data from Old db...");
    let (categories, mut products) = query_all().await?;

    let mut root = YamlCategories {
        meno: "root".to_owned(),
        meno_typu: "".to_owned(),
        podkategorie: vec![],
        id: u32::MAX,
    };

    //First push the categories with no parents
    for cat in &categories {
        if cat.try_borrow()?.parent_category.is_none() {
            let curr_cat = cat.try_borrow()?;
            root.podkategorie.push(YamlCategories {
                meno: curr_cat
                    .category
                    .try_borrow()?
                    .name
                    .clone()
                    .trim()
                    .to_string(),
                meno_typu: "".to_owned(),
                podkategorie: vec![],
                id: curr_cat.category.try_borrow()?.id.clone(),
            });
        }
    }

    //Now keep looping and increasing depth of categories
    let mut current_layer = root
        .podkategorie
        .iter_mut()
        .collect::<Vec<&mut YamlCategories>>();
    loop {
        let mut next_layer = vec![];
        for cat in current_layer {
            println!("Matching {}...", cat.meno);
            for old_cat in &categories {
                if let Some(parent) = &old_cat.try_borrow()?.parent_category {
                    if parent.try_borrow()?.category.try_borrow()?.id == cat.id {
                        let old_cat = old_cat.try_borrow()?;
                        let old_cat = old_cat.category.try_borrow()?;
                        println!("{} - IS PARENT OF - {}!", cat.meno, &old_cat.name);
                        let new_cat = YamlCategories {
                            id: old_cat.id.clone(),
                            meno: old_cat.name.clone().trim().to_string(),
                            podkategorie: vec![],
                            meno_typu: "".to_owned(),
                        };
                        let podkategorie = &mut cat.podkategorie;
                        podkategorie.push(new_cat);
                        next_layer.push(old_cat.id.clone());
                    }
                }
            }
        }
        current_layer = root.get_last_layer(next_layer);
        next_layer = vec![];
        if current_layer.is_empty() {
            break;
        }
    }
    let yaml = serde_yaml::to_string(&root)?;
    std::fs::write("katekórie.yaml", yaml)?;
    anyhow::Ok(())
}
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct YamlCategories {
    pub meno_typu: String,
    pub meno: String,
    id: u32,
    pub podkategorie: Vec<Self>,
}

impl YamlCategories {
    pub fn get_last_layer(&mut self, ids: Vec<u32>) -> Vec<&mut Self> {
        let mut result: Vec<&mut Self> = vec![];

        for cat in self.podkategorie.iter_mut() {
            let last_subcategory_less_cat = cat;
            if !last_subcategory_less_cat.podkategorie.is_empty() {
                let find = last_subcategory_less_cat.get_last_layer(ids.clone());
                for f in find {
                    if ids.contains(&f.id) {
                        result.push(f)
                    }
                }
            } else {
                if ids.contains(&last_subcategory_less_cat.id) {
                    result.push(last_subcategory_less_cat);
                }
            }
        }
        result
    }
}
