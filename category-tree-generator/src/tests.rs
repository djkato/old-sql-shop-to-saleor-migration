#[cfg(test)]
use crate::query_all;
#[tokio::test]
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
