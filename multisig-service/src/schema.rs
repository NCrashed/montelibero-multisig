table! {
    transaction_updates (id) {
        id -> Integer,
        txid -> Text,
        body -> Binary,
        updated -> Timestamp,
    }
}

table! {
    transactions (id) {
        id -> Text,
        title -> Text,
        description -> Text,
        body -> Binary,
        created -> Timestamp,
    }
}

joinable!(transaction_updates -> transactions (txid));

allow_tables_to_appear_in_same_query!(transaction_updates, transactions,);
