#!/usr/bin/env node
// scripts/build-fix.js
import { spawn } from "child_process";
import process from "process";

// Ensure the required variables are set.
process.env.NAPI_RS_LINK_TYPE = "dynamic";
process.env.NAPI_RS_DYNAMIC_RUNTIME = "yes";

const child = spawn("napi", ["build", "--platform", "--release"], {
  stdio: "inherit",
  shell: true,
  env: process.env,
});

child.on("exit", (code) => process.exit(code));
