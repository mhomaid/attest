import { ImageResponse } from "next/og";

export const alt = "Attest: every AI verdict is a signed object you can check";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

const rows = [
  { id: "#41", hash: "e3b0…c442", tone: "#3ddc84" },
  { id: "#42", hash: "a91c…07fe", tone: "#ef4444" },
  { id: "#43", hash: "7be2…d19a", tone: "#3ddc84" },
];

export default function OpengraphImage() {
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        padding: 72,
        background: "linear-gradient(135deg, #07090c 0%, #0c1510 60%, #07090c 100%)",
        color: "#f5f7f6",
        fontFamily: "sans-serif",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 16, fontSize: 30, fontWeight: 700 }}>
        <div
          style={{
            width: 44,
            height: 44,
            borderRadius: 10,
            border: "2px solid #1e90ff",
            display: "flex",
          }}
        />
        Attest
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
        <div style={{ fontSize: 68, fontWeight: 800, lineHeight: 1.05, letterSpacing: -1.5 }}>
          Every AI verdict is a signed object you can check.
        </div>
        <div style={{ fontSize: 28, color: "#9aa3a0" }}>
          Change one field and attest verify fails.
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 18 }}>
        {rows.map((r, i) => (
          <div key={r.id} style={{ display: "flex", alignItems: "center", gap: 18 }}>
            {i > 0 ? <div style={{ width: 48, height: 2, background: "#2a3330", display: "flex" }} /> : null}
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                padding: "12px 20px",
                borderRadius: 12,
                border: `2px solid ${r.tone}`,
                fontFamily: "monospace",
              }}
            >
              <span style={{ fontSize: 24, fontWeight: 700 }}>{r.id}</span>
              <span style={{ fontSize: 16, color: "#9aa3a0" }}>{r.hash}</span>
            </div>
          </div>
        ))}
        <div style={{ display: "flex", marginLeft: 28, fontSize: 22, color: "#ef4444", fontFamily: "monospace" }}>
          FAIL chain break
        </div>
      </div>
    </div>,
    size,
  );
}
