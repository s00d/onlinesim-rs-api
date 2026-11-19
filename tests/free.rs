mod common;

use common::{client, fixture, ok, q};
use onlinesim_rs_api::FreeListOptions;
use serde_json::json;

#[tokio::test]
async fn list_has_no_php_suffix_and_sends_only_the_set_parameters() {
    let (client, fake) = client(vec![ok(fixture("getFreeList"))]);
    let list = client
        .free()
        .list(&FreeListOptions {
            country: Some(44),
            number: Some("7459608320".into()),
            count: Some(20),
            ..FreeListOptions::default()
        })
        .await
        .unwrap();
    let call = fake.last();
    assert_eq!(call.path(), "/api/getFreeList");
    assert_eq!(
        call.query(),
        q(&[("country", "44"), ("number", "7459608320"), ("count", "20")])
    );
    assert_eq!(list.countries.len(), 2);
    assert_eq!(list.numbers["7459608320"].full_number, "447459608320");
    assert_eq!(list.messages.page.data[0].code, "195895");
    assert_eq!(list.messages.page.total, 426);
    assert_eq!(list.messages.number.as_deref(), Some("7459608320"));
    assert_eq!(list.messages.country, 44);
}

#[tokio::test]
async fn list_normalises_empty_numbers() {
    let (client, fake) = client(vec![ok(json!({
        "response": 1, "countries": [], "numbers": [], "messages": { "data": [] }
    }))]);
    let list = client
        .free()
        .list(&FreeListOptions::default())
        .await
        .unwrap();
    assert!(list.numbers.is_empty());
    assert!(fake.last().query().is_empty());
}
