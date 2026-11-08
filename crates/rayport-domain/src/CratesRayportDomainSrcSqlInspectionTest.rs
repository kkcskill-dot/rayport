use super::*;

const SAFE: &str = "SELECT business_date, store_id, SUM(amount) AS total FROM sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND store_id = :store_id GROUP BY business_date, store_id";

#[test]
fn accepts_bounded_template_and_extracts_non_secret_metadata() {
    let result = inspect_sql(SAFE).unwrap();
    assert_eq!(result.tables, vec!["SALES"]);
    assert_eq!(
        result.parameters,
        vec!["end_date_exclusive", "start_date", "store_id"]
    );
    assert_eq!(result.fingerprint.len(), 64);
}

#[test]
fn canonicalization_groups_whitespace_case_and_trailing_semicolon() {
    let other = SAFE.to_lowercase().replace(" from ", "\nfrom\n") + ";";
    assert_eq!(
        inspect_sql(SAFE).unwrap().fingerprint,
        inspect_sql(&other).unwrap().fingerprint
    );
}

#[test]
fn comments_literals_and_potentially_sensitive_values_are_never_returned() {
    for sql in [
        "SELECT amount FROM sales WHERE store_id = 'customer-secret'",
        "SELECT amount FROM sales WHERE account_id = 123456789",
        "SELECT amount FROM sales WHERE business_date = DATE '2026-01-01'",
        "SELECT amount FROM sales WHERE store_id = :store_id -- comment",
        "SELECT /*+ PARALLEL(32) */ amount FROM sales WHERE store_id = :store_id",
        "SELECT amount FROM sales WHERE store_id = :store_id /* comment */",
    ] {
        let error = inspect_sql(sql).unwrap_err();
        assert_eq!(error.code, "unsafe_sql");
        assert!(!error.message.contains("customer-secret"));
        assert!(!error.message.contains("123456789"));
    }
}

#[test]
fn rejects_write_lock_multi_statement_and_cte_constructs() {
    for sql in [
        "DELETE FROM sales WHERE store_id = :store_id",
        "SELECT amount FROM sales WHERE store_id = :store_id; DELETE FROM sales",
        "SELECT amount FROM sales WHERE store_id = :store_id FOR UPDATE",
        "SELECT amount FROM sales WHERE store_id = :store_id FOR SHARE",
        "SELECT amount INTO archive FROM sales WHERE store_id = :store_id",
        "WITH changed AS (DELETE FROM sales RETURNING amount) SELECT amount FROM changed",
        "WITH s AS (SELECT amount FROM sales) SELECT amount FROM s WHERE amount > :amount",
        "SELECT amount FROM sales WHERE store_id = :store_id UNION SELECT amount FROM archive",
        "SELECT amount FROM sales WHERE EXISTS (SELECT value FROM archive)",
    ] {
        assert!(inspect_sql(sql).is_err(), "unexpected accepted SQL: {sql}");
    }
}

#[test]
fn rejects_function_side_effects_and_nested_bypasses() {
    for sql in [
        "SELECT pg_sleep(:delay) FROM sales WHERE store_id = :store_id",
        "SELECT SUM(evil(amount)) FROM sales WHERE store_id = :store_id",
        "SELECT sys.utl_http.request(:url) FROM sales WHERE store_id = :store_id",
        "SELECT seq.nextval FROM sales WHERE store_id = :store_id",
        "SELECT amount FROM read_file(:file) WHERE store_id = :store_id",
        "SELECT amount FROM sales WHERE store_id = :store_id AND mutate(amount) = :amount",
        "SELECT amount FROM sales WHERE store_id = :store_id OR store_id = :other",
        "SELECT CAST(amount AS suspicious) FROM sales WHERE store_id = :store_id",
        "SELECT amount FROM (SELECT amount FROM sales) sub WHERE amount = :amount",
    ] {
        assert!(inspect_sql(sql).is_err(), "unexpected accepted SQL: {sql}");
    }
}

#[test]
fn rejects_wildcard_unbounded_and_dynamic_identifier_forms() {
    for sql in [
        "SELECT * FROM sales WHERE store_id = :store_id",
        "SELECT amount FROM sales",
        "SELECT amount FROM sales WHERE store_id = ?",
        "SELECT amount FROM sales WHERE store_id = $1",
        "SELECT amount FROM sales WHERE store_id = :1",
        "SELECT amount FROM sales@remote WHERE store_id = :store_id",
        "SELECT \"amount\" FROM sales WHERE store_id = :store_id",
        "SELECT amount FROM sales CROSS JOIN archive WHERE store_id = :store_id",
    ] {
        assert!(inspect_sql(sql).is_err(), "unexpected accepted SQL: {sql}");
    }
}

#[test]
fn accepts_reviewable_simple_join_and_count() {
    let sql = "SELECT s.store_id, COUNT(*) AS total FROM sales s INNER JOIN stores t ON s.store_id = t.store_id WHERE s.store_id = :store_id GROUP BY s.store_id";
    assert_eq!(inspect_sql(sql).unwrap().tables, vec!["SALES", "STORES"]);
}

#[test]
fn input_size_is_bounded_without_panicking() {
    assert!(inspect_sql(&" ".repeat(32769)).is_err());
    assert!(inspect_sql("SELECT 매출 FROM sales WHERE store_id = :store_id").is_err());
    let nested = format!(
        "SELECT {}amount{} FROM sales WHERE store_id = :store_id",
        "(".repeat(500),
        ")".repeat(500)
    );
    assert!(inspect_sql(&nested).is_err());
}

#[test]
fn required_binds_in_projection_or_tautologies_do_not_limit_execution() {
    assert!(validate_apic_bind_guards(SAFE).is_ok());
    for sql in [
        "SELECT :start_date, :end_date_exclusive, :store_id FROM sales WHERE amount > :amount",
        "SELECT amount FROM sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND :store_id = :store_id",
        "SELECT amount FROM sales WHERE business_date >= :start_date AND business_date <= :end_date_exclusive AND store_id = :store_id",
    ] {
        assert!(inspect_sql(sql).is_ok());
        assert_eq!(validate_apic_bind_guards(sql).unwrap_err().code, "missing_bind_guard");
    }
}

#[test]
fn unknown_function_argument_shapes_fail_closed() {
    for sql in [
        "SELECT SUM(*) FROM sales WHERE store_id = :store_id",
        "SELECT MIN(amount, other) FROM sales WHERE store_id = :store_id",
        "SELECT COUNT(sales.*) FROM sales WHERE store_id = :store_id",
    ] {
        assert!(inspect_sql(sql).is_err());
    }
}

#[test]
fn long_left_associative_expression_chains_are_bounded() {
    let sql = format!(
        "SELECT amount FROM sales WHERE {}",
        std::iter::repeat_n("amount = :amount", 300)
            .collect::<Vec<_>>()
            .join(" AND ")
    );
    assert!(inspect_sql(&sql).is_err());
}
