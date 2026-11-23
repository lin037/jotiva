import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { parseEnv } from "node:util";

const env = { ...process.env };
if (!env.JOT_JEV_API_KEY) {
  const file = new URL("../.env", import.meta.url);
  const key = env.JEV_API_KEY ?? (existsSync(file) && parseEnv(readFileSync(file, "utf8")).JEV_API_KEY);
  if (!key) throw Error("请在项目 .env 中设置 JEV_API_KEY，或为自定义端点设置 JOT_JEV_API_KEY");
  env.JOT_JEV_ENDPOINT = "https://api.typesafe.ai/v1/systemone";
  env.JOT_JEV_API_KEY = key;
}
const result = spawnSync("cargo", [
  "test", "--locked", "--manifest-path", "src-tauri/Cargo.toml",
  "settings::filing::tests::live::classifies_synthetic_notes", "--", "--ignored", "--exact",
], { env, stdio: "inherit" });
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
