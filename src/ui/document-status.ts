import { t } from "../i18n";
import { element } from "./dom";
import "./document-status.css";

export function documentStatus() {
  const footer = element("footer", "workspace-footer jot-statusbar");
  const metadata = element("div", "workspace-metadata");
  const stats = element("div", "workspace-stats");
  const counter = element("span", "workspace-count");
  const status = element("span", "muted", t("正在打开…"));
  status.setAttribute("role", "status");
  stats.append(counter, status);
  footer.append(metadata, stats);
  return { footer, metadata, counter, status };
}
