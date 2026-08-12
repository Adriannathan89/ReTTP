use rettp_reporter::{
    JsonReporter, REPORT_SCHEMA_VERSION, ReportBlock, ReportBlockKind, ReportStatus, ReportSummary,
    ReportTest, RunReport,
};

#[test]
fn renders_pretty_json_with_stable_order_and_exactly_one_final_newline() {
    let report = RunReport {
        schema_version: REPORT_SCHEMA_VERSION,
        source: "tests/api.rttp".into(),
        name: Some("API".into()),
        status: ReportStatus::Passed,
        duration_ms: 42,
        summary: ReportSummary {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
            aborted: 0,
        },
        blocks: vec![ReportBlock {
            kind: ReportBlockKind::Test,
            name: Some("health".into()),
            status: ReportStatus::Passed,
            duration_ms: 42,
            tests: vec![ReportTest {
                name: "health".into(),
                status: ReportStatus::Passed,
                duration_ms: 42,
                failures: Vec::new(),
                error: None,
            }],
        }],
        error: None,
    };

    let output = JsonReporter.render(&report).unwrap();

    assert!(output.ends_with("\n"));
    assert!(!output.ends_with("\n\n"));
    assert_eq!(
        output,
        concat!(
            "{\n",
            "  \"schema_version\": 1,\n",
            "  \"source\": \"tests/api.rttp\",\n",
            "  \"name\": \"API\",\n",
            "  \"status\": \"passed\",\n",
            "  \"duration_ms\": 42,\n",
            "  \"summary\": {\n",
            "    \"total\": 1,\n",
            "    \"passed\": 1,\n",
            "    \"failed\": 0,\n",
            "    \"skipped\": 0,\n",
            "    \"aborted\": 0\n",
            "  },\n",
            "  \"blocks\": [\n",
            "    {\n",
            "      \"kind\": \"test\",\n",
            "      \"name\": \"health\",\n",
            "      \"status\": \"passed\",\n",
            "      \"duration_ms\": 42,\n",
            "      \"tests\": [\n",
            "        {\n",
            "          \"name\": \"health\",\n",
            "          \"status\": \"passed\",\n",
            "          \"duration_ms\": 42,\n",
            "          \"failures\": [],\n",
            "          \"error\": null\n",
            "        }\n",
            "      ]\n",
            "    }\n",
            "  ],\n",
            "  \"error\": null\n",
            "}\n"
        )
    );
}

#[test]
fn json_renderer_has_expected_value_semantics() {
    let reporter = JsonReporter;
    assert_eq!(reporter, JsonReporter);
    assert!(format!("{reporter:?}").contains("JsonReporter"));
}
