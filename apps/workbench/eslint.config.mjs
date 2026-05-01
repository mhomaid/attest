import nextVitals from "eslint-config-next/core-web-vitals";
import nextTypescript from "eslint-config-next/typescript";

/** TanStack Table returns unstable function refs; React Compiler skips memoization — expected. */
const tanstackTableOverride = {
  files: [
    "src/components/data-table.tsx",
    "src/components/workbench/alert-queue.tsx",
    "src/components/workbench/attestation-trace-panel.tsx",
  ],
  rules: {
    "react-hooks/incompatible-library": "off",
  },
};

const eslintConfig = [...nextVitals, ...nextTypescript, tanstackTableOverride];

export default eslintConfig;
