#[cfg(test)]
use crate::query_all;

/*
#[test]
fn category_typename_algorithm() {
    use crate::get_sqls::{FinalProductType, YamlCategories};
    let yaml_cats = YamlCategories::load();

    assert_eq!(
        Some(FinalProductType {
            name: "Batéria".to_owned(),
            saleor_id: None
        }),
        yaml_cats.find_product_type(697)
    );
    assert_eq!(
        Some(FinalProductType {
            name: "FAQ".to_owned(),
            saleor_id: None
        }),
        yaml_cats.find_product_type(1914)
    );
    assert_eq!(
        Some(FinalProductType {
            name: "Niť".to_owned(),
            saleor_id: None
        }),
        yaml_cats.find_product_type(1795)
    );
}
*/

/*#[tokio::test]
async fn data_relations() {
    let (categories, products) = query_all().await.unwrap();
    let product1 = products
        .iter()
        .find(|product| product.product.id == 18732)
        .unwrap();

    assert_eq!(
        product1
            .category
            .as_ref()
            .unwrap()
            .borrow()
            .category
            .borrow()
            .id,
        2161
    );
    assert_eq!(
        product1
            .category
            .as_ref()
            .unwrap()
            .borrow()
            .parent_category
            .as_ref()
            .unwrap()
            .borrow()
            .category
            .borrow()
            .id,
        352
    );
    assert_eq!(
        product1
            .category
            .as_ref()
            .unwrap()
            .borrow()
            .parent_category
            .as_ref()
            .unwrap()
            .borrow()
            .parent_category
            .as_ref()
            .unwrap()
            .borrow()
            .category
            .borrow()
            .id,
        1217
    );
}
    */
