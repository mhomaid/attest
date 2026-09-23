/// <reference types="bun" />
import { describe, expect, test } from "bun:test";
import { firedDetectionToAlert, parseFiredDetections } from "@/lib/detection-to-alert";
import { ocsfEventToAlert, parseEventsResponse } from "@/lib/ocsf-to-alert";

describe("firedDetectionToAlert", () => {
  const row = {
    detection_id: "aws_console_login_from_anomalous_geolocation",
    event_id: "e-123",
    actor_user_name: "alice@corp.com",
    cloud_region: "ap-southeast-1",
    severity: "HIGH",
    fired_at: "2026-04-27 05:36:45.785834+00:00",
  };

  test("maps a known detection to its title and MITRE technique", () => {
    const alert = firedDetectionToAlert(row);
    expect(alert).toMatchObject({
      id: "e-123",
      caseId: "e-123",
      title: "Console Login from Anomalous Region",
      technique: "T1078.004",
      severity: "high",
      entity: "alice@corp.com",
      region: "ap-southeast-1",
      state: "awaiting-review",
    });
  });

  test("falls back to a title-cased id, no technique, and medium severity", () => {
    const alert = firedDetectionToAlert({
      ...row,
      detection_id: "custom_rule_x",
      severity: "urgent",
      actor_user_name: "",
      cloud_region: "",
    });
    expect(alert).toMatchObject({
      title: "Custom Rule X",
      technique: "",
      severity: "medium",
      entity: "unknown",
      region: "unknown",
    });
  });

  test("parseFiredDetections rejects non-array payloads", () => {
    expect(parseFiredDetections({ error: "boom" })).toEqual([]);
    expect(parseFiredDetections([row])).toHaveLength(1);
  });
});

describe("ocsfEventToAlert", () => {
  test("titles authentication events by outcome and region", () => {
    const ok = ocsfEventToAlert(
      { event_id: "abcdef1234", class_uid: "3002", auth_status: "Success", cloud_region: "us-east-1" },
      0,
    );
    expect(ok.title).toBe("successful console login (us-east-1)");
    expect(ok.technique).toBe("T1078.004");

    const failed = ocsfEventToAlert({ event_id: "abcdef1234", class_uid: "3002" }, 0);
    expect(failed.title).toBe("failed console login");
  });

  test("titles API activity by operation and service, else by event id", () => {
    expect(
      ocsfEventToAlert({ event_id: "e1", api_operation: "PutBucketPolicy", api_service: "s3" }, 0)
        .title,
    ).toBe("PutBucketPolicy via s3");
    expect(ocsfEventToAlert({ event_id: "abcdef1234" }, 0).title).toBe("OCSF event abcdef12");
  });

  test("defaults unknown severity to low and prefers user name over uid", () => {
    const alert = ocsfEventToAlert(
      { event_id: "e1", severity: "bogus", actor_user_uid: "u-1", actor_user_name: "bob" },
      4,
    );
    expect(alert.severity).toBe("low");
    expect(alert.entity).toBe("bob");
    expect(alert.updatedAt).toBe("event 5");
  });

  test("parseEventsResponse accepts an array or a single event and ignores garbage", () => {
    expect(parseEventsResponse([{ event_id: "a" }, { event_id: "b" }])).toHaveLength(2);
    expect(parseEventsResponse({ event_id: "a" })).toHaveLength(1);
    expect(parseEventsResponse({ nope: true })).toEqual([]);
    expect(parseEventsResponse(null)).toEqual([]);
  });
});
