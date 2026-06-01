use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, ValueEnum};
use qexed_config::{
    app::{
        qexed::{Qexed, server::World},
        qexed_content_filter::QexedContentFilter,
        qexed_entity::QexedEntity,
        qexed_entity_rendering::QexedEntityRendering,
        qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
        qexed_lan_discovery::QexedLanDiscovery,
        qexed_lobby::QexedLobby,
        qexed_menus::QexedMenus,
        qexed_npc::QexedNpc,
        qexed_permissions::QexedPermissions,
        qexed_placeholders::QexedPlaceholders,
        qexed_player_audit::QexedPlayerAudit,
        qexed_player_data::QexedPlayerData,
        qexed_player_messages::QexedPlayerMessages,
        qexed_plugin_download::QexedPluginDownload,
        qexed_proxy::QexedProxy,
        qexed_resource_pack::QexedResourcePack,
        qexed_scoreboard::QexedScoreboard,
        qexed_server::QexedServer,
        qexed_warden::QexedWarden,
    },
    build,
    tool::{AppConfigTrait, AutoDocConfigTrait},
};
use qexed_plugin_api::{PlaceholderDoc, PlaceholderScope, PluginEvent, native_placeholder_docs};
use serde::Serialize;
use serde_json::{Value as JsonValue, json};

const DEFAULT_LANGS: &[&str] = &["zh-CN", "en"];
const LOGO_BYTES: &[u8] = include_bytes!("../../qexed_doc_logo.ico");
const CONFIG_DOC_CLIENT_TSX: &str = r###""use client";

import {
  Fragment,
  createContext,
  useContext,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type UIEvent,
  type ReactNode,
} from "react";
import { usePathname } from "next/navigation";

type ConfigField = {
  path: string;
  description: string;
  value_type: string;
  default_value: string | null;
  warning: string | null;
  danger: string | null;
  pending_deprecated: string | null;
  deprecated: string | null;
  migration_notice: string | null;
};

type ConfigDocLabels = {
  path: string;
  valueType: string;
  defaultValue: string;
  currentValue: string;
  description: string;
  notice: string;
  none: string;
  notLoaded: string;
  missing: string;
  editorTitle: string;
  editorSubtitle: string;
  upload: string;
  clear: string;
  copy: string;
  copied: string;
  pastePlaceholder: string;
  parseOk: string;
  parseError: string;
  matchedFields: string;
  fileLabel: string;
  complexDetails: string;
  detailsLink: string;
  viewDetails: string;
  fieldUnit: string;
  danger: string;
  warning: string;
  pendingDeprecated: string;
  deprecated: string;
  migration: string;
};

type ConfigDocContextValue = {
  appName: string;
  configPath: string;
  configDocs: ConfigDocFile[];
  commit: string;
  lang: string;
  fields: ConfigField[];
  labels: ConfigDocLabels;
  source: string;
  values: Record<string, unknown> | null;
  error: string | null;
  matchedFieldCount: number;
  setSourceText: (source: string) => void;
};

type ConfigDocFile = {
  name: string;
  configPath: string;
  route: string;
  description: string;
};

type ConfigTreeNode = {
  name: string;
  path: string;
  children: ConfigTreeNode[];
  doc?: ConfigDocFile;
};

type LookupResult = {
  found: boolean;
  value?: unknown;
};

type RichTextPart =
  | {
      type: "text";
      text: string;
    }
  | {
      type: "code";
      language: string;
      code: string;
    };

const DEFAULT_LABELS: ConfigDocLabels = {
  path: "Path",
  valueType: "Type",
  defaultValue: "Default",
  currentValue: "Current",
  description: "Description",
  notice: "Notice",
  none: "None",
  notLoaded: "Paste or upload TOML to preview current values.",
  missing: "Not provided",
  editorTitle: "TOML preview",
  editorSubtitle: "Paste or upload a config file. Matching fields update in the table.",
  upload: "Upload TOML",
  clear: "Clear",
  copy: "Copy",
  copied: "Copied",
  pastePlaceholder: "Paste TOML here...",
  parseOk: "TOML parsed",
  parseError: "TOML parse error",
  matchedFields: "matched fields",
  fileLabel: "TOML",
  complexDetails: "Complex Type Details",
  detailsLink: "See #{path}",
  viewDetails: "View details",
  fieldUnit: "fields",
  danger: "Danger",
  warning: "Warning",
  pendingDeprecated: "Pending deprecated",
  deprecated: "Deprecated",
  migration: "Migration",
};

const ConfigDocContext = createContext<ConfigDocContextValue | null>(null);

export function ConfigDocProvider({
  appName,
  configPath,
  configDocs,
  commit,
  lang,
  fields,
  labels,
  children,
}: {
  appName: string;
  configPath: string;
  configDocs: ConfigDocFile[];
  commit: string;
  lang: string;
  fields: ConfigField[];
  labels: ConfigDocLabels;
  children: ReactNode;
}) {
  const mergedLabels = useMemo(
    () => ({
      ...DEFAULT_LABELS,
      ...labels,
    }),
    [labels],
  );
  const [source, setSource] = useState("");
  const [parseState, setParseState] = useState<{
    values: Record<string, unknown> | null;
    error: string | null;
  }>({ values: null, error: null });

  const setSourceText = (nextSource: string) => {
    setSource(nextSource);
    if (!nextSource.trim()) {
      setParseState({ values: null, error: null });
      return;
    }

    try {
      setParseState({ values: parseToml(nextSource), error: null });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setParseState({ values: null, error: message });
    }
  };

  const matchedFieldCount = useMemo(() => {
    const values = parseState.values;
    if (!values) {
      return 0;
    }

    return fields.filter((field) => getValueAtPath(values, field.path).found).length;
  }, [fields, parseState.values]);

  const value = useMemo<ConfigDocContextValue>(
    () => ({
      appName,
      configPath,
      configDocs,
      commit,
      lang,
      fields,
      labels: mergedLabels,
      source,
      values: parseState.values,
      error: parseState.error,
      matchedFieldCount,
      setSourceText,
    }),
    [
      appName,
      configPath,
      configDocs,
      commit,
      lang,
      fields,
      mergedLabels,
      source,
      parseState.values,
      parseState.error,
      matchedFieldCount,
    ],
  );

  return (
    <ConfigDocContext.Provider value={value}>
      <ConfigDocStyles />
      {children}
    </ConfigDocContext.Provider>
  );
}

export function ConfigDocEditor() {
  const {
    configPath,
    configDocs,
    fields,
    labels,
    source,
    values,
    error,
    matchedFieldCount,
    setSourceText,
  } = useConfigDoc();
  const lineNumberRef = useRef<HTMLPreElement>(null);

  const lineNumbers = useMemo(() => {
    const count = Math.max(1, source.split(/\r?\n/).length);
    return Array.from({ length: count }, (_, index) => index + 1).join("\n");
  }, [source]);

  const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
    setSourceText(event.target.value);
  };

  const handleScroll = (event: UIEvent<HTMLTextAreaElement>) => {
    if (lineNumberRef.current) {
      lineNumberRef.current.scrollTop = event.currentTarget.scrollTop;
    }
  };

  const handleUpload = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) {
      return;
    }

    void file.text().then((content) => {
      setSourceText(content);
    });
  };

  const status = error
    ? `${labels.parseError}: ${error}`
    : values
      ? `${labels.parseOk}: ${matchedFieldCount}/${fields.length} ${labels.matchedFields}`
      : labels.notLoaded;

  return (
    <section className="qexed-vscode-shell" aria-label={labels.editorTitle}>
      <div className="qexed-vscode-activity" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
      <div className="qexed-vscode-sidebar">
        <ConfigFileExplorer configDocs={configDocs} configPath={configPath} />
      </div>
      <div className="qexed-vscode-main">
        <div className="qexed-vscode-titlebar">
          <div>
            <div className="qexed-vscode-title">{labels.editorTitle}</div>
            <div className="qexed-vscode-subtitle">{labels.editorSubtitle}</div>
          </div>
          <div className="qexed-vscode-actions">
            <label className="qexed-vscode-button">
              {labels.upload}
              <input accept=".toml,text/toml,text/plain" onChange={handleUpload} type="file" />
            </label>
            <CopySourceButton />
            <button className="qexed-vscode-button" onClick={() => setSourceText("")} type="button">
              {labels.clear}
            </button>
          </div>
        </div>
        <div className="qexed-vscode-tabbar">
          <span className="qexed-vscode-tab">{fileNameFromPath(configPath) || labels.fileLabel}</span>
        </div>
        <div className="qexed-vscode-editor">
          <pre className="qexed-vscode-lines" aria-hidden="true" ref={lineNumberRef}>
            {lineNumbers}
          </pre>
          <textarea
            aria-label={labels.editorTitle}
            className="qexed-vscode-textarea"
            onChange={handleChange}
            onScroll={handleScroll}
            placeholder={labels.pastePlaceholder}
            spellCheck={false}
            value={source}
          />
        </div>
        <div className={`qexed-vscode-status ${error ? "is-error" : values ? "is-ok" : ""}`}>
          {status}
        </div>
      </div>
    </section>
  );
}

function ConfigFileExplorer({
  configDocs,
  configPath,
}: {
  configDocs: ConfigDocFile[];
  configPath: string;
}) {
  const pathname = usePathname();
  const currentPath = normalizeConfigPath(configPath);
  const configTree = useMemo(() => buildConfigTree(configDocs), [configDocs]);
  const basePath = useMemo(() => {
    const normalizedPath = pathname.replace(/\/$/, "");
    const segments = normalizedPath.split("/").filter(Boolean);
    segments.pop();
    return segments.length > 0 ? `/${segments.join("/")}` : "/";
  }, [pathname]);

  return (
    <>
      <div className="qexed-vscode-sidebar-header">
        <div className="qexed-vscode-sidebar-title">
          <span className="qexed-vscode-tree-arrow is-open" aria-hidden="true" />
          RUN
        </div>
        <div className="qexed-vscode-sidebar-actions" aria-hidden="true">
          <span />
          <span />
          <span />
          <span />
        </div>
      </div>
      <div className="qexed-vscode-tree" aria-label="RUN">
        <ConfigTreeNodes basePath={basePath} currentPath={currentPath} nodes={configTree} />
        {["logs", "plugins", "resourcepacks", "world"].map((folder) => (
          <div className="qexed-vscode-tree-folder" key={folder}>
            <span className="qexed-vscode-tree-arrow" aria-hidden="true" />
            <span className="qexed-vscode-folder-icon" aria-hidden="true" />
            <span>{folder}</span>
          </div>
        ))}
        {["qexed.exe"].map((file) => (
          <div className="qexed-vscode-tree-file" key={file}>
            <span className="qexed-vscode-file-icon" aria-hidden="true" />
            <span>{file}</span>
          </div>
        ))}
      </div>
    </>
  );
}

function ConfigTreeNodes({
  basePath,
  currentPath,
  nodes,
}: {
  basePath: string;
  currentPath: string;
  nodes: ConfigTreeNode[];
}) {
  return (
    <>
      {nodes.map((node) =>
        node.doc ? (
          <a
            aria-current={normalizeConfigPath(node.doc.configPath) === currentPath ? "page" : undefined}
            className={`qexed-vscode-tree-file ${normalizeConfigPath(node.doc.configPath) === currentPath ? "is-active" : ""}`}
            href={`${basePath === "/" ? "" : basePath}/${node.doc.route}`}
            key={node.path}
            title={node.doc.description}
          >
            <span className="qexed-vscode-file-icon" aria-hidden="true" />
            <span>{node.name}</span>
          </a>
        ) : (
          <Fragment key={node.path}>
            <div className="qexed-vscode-tree-folder is-open">
              <span className="qexed-vscode-tree-arrow is-open" aria-hidden="true" />
              <span className="qexed-vscode-folder-icon" aria-hidden="true" />
              <span>{node.name}</span>
            </div>
            <div className="qexed-vscode-tree-group">
              <ConfigTreeNodes basePath={basePath} currentPath={currentPath} nodes={node.children} />
            </div>
          </Fragment>
        ),
      )}
    </>
  );
}

function buildConfigTree(configDocs: ConfigDocFile[]) {
  const nodes: ConfigTreeNode[] = [];

  for (const doc of configDocs) {
    const parts = normalizeConfigPath(doc.configPath).split("/").filter(Boolean);
    let siblings = nodes;
    let currentPath = "";

    parts.forEach((part, index) => {
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      let node = siblings.find((candidate) => candidate.name === part);
      if (!node) {
        node = { name: part, path: currentPath, children: [] };
        siblings.push(node);
      }

      if (index === parts.length - 1) {
        node.doc = doc;
      } else {
        siblings = node.children;
      }
    });
  }

  return sortConfigTree(nodes);
}

function sortConfigTree(nodes: ConfigTreeNode[]): ConfigTreeNode[] {
  return nodes
    .map((node) => ({
      ...node,
      children: sortConfigTree(node.children),
    }))
    .sort((left, right) => {
      if (Boolean(left.doc) !== Boolean(right.doc)) {
        return left.doc ? 1 : -1;
      }

      return left.name.localeCompare(right.name);
    });
}

function normalizeConfigPath(path: string) {
  return path.replaceAll("\\", "/").split("/").filter(Boolean).join("/");
}

function fileNameFromPath(path: string) {
  return normalizeConfigPath(path).split("/").at(-1) || "";
}

export function ConfigFieldTable() {
  const { fields, labels } = useConfigDoc();
  const topLevelFields = fields.filter((field) => isTopLevelField(field, fields));

  return (
    <div className="qexed-config-table-wrap">
      <table className="qexed-config-table">
        <thead>
          <tr>
            <th>{labels.path}</th>
            <th>{labels.valueType}</th>
            <th>{labels.defaultValue}</th>
            <th>{labels.currentValue}</th>
            <th>{labels.description}</th>
            <th>{labels.notice}</th>
          </tr>
        </thead>
        <tbody>
          {topLevelFields.map((field) => (
            <tr key={field.path}>
              <td>
                <code>{field.path}</code>
              </td>
              <td>
                <code>{field.value_type}</code>
              </td>
              <td>
                <DefaultValue field={field} linkComplex={hasDirectChildren(field, fields)} />
              </td>
              <td>
                <CurrentValue field={field} linkComplex={hasDirectChildren(field, fields)} />
              </td>
              <td>
                <RichText text={field.description} />
              </td>
              <td>
                <NoticeList field={field} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function ConfigComplexDetails() {
  const { fields, labels } = useConfigDoc();
  const complexFields = fields.filter((field) => isComplexField(field) && hasDirectChildren(field, fields));

  if (complexFields.length === 0) {
    return null;
  }

  return (
    <section className="qexed-complex-details">
      <h2>{labels.complexDetails}</h2>
      {complexFields.map((field) => {
        const childPrefix = `${field.path}.`;
        const children = fields.filter((candidate) => isDirectChildPath(candidate.path, childPrefix));

        return (
          <section className="qexed-complex-field" id={detailsId(field.path)} key={field.path}>
            <h3>
              <code>{field.path}</code>
            </h3>
            <dl>
              <div>
                <dt>{labels.valueType}</dt>
                <dd>
                  <code>{field.value_type}</code>
                </dd>
              </div>
              <div>
                <dt>{labels.defaultValue}</dt>
                <dd>
                  {children.length > 0 ? <span className="qexed-muted">{labels.none}</span> : <DefaultValue field={field} />}
                </dd>
              </div>
              <div>
                <dt>{labels.currentValue}</dt>
                <dd>
                  <CurrentValue field={field} />
                </dd>
              </div>
              <div>
                <dt>{labels.description}</dt>
                <dd>
                  <RichText text={field.description} />
                </dd>
              </div>
            </dl>
            <NoticeList field={field} />
            {children.length > 0 ? <NestedFieldTable allFields={fields} fields={children} /> : null}
          </section>
        );
      })}
    </section>
  );
}

export function CopyableCodeBlock({
  code,
  language = "text",
  compact = false,
}: {
  code: string;
  language?: string;
  compact?: boolean;
}) {
  const labels = useContext(ConfigDocContext)?.labels ?? DEFAULT_LABELS;
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    if (typeof navigator === "undefined" || !navigator.clipboard) {
      return;
    }

    await navigator.clipboard.writeText(code);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };

  return (
    <div className={`qexed-code-block ${compact ? "is-compact" : ""}`}>
      <div className="qexed-code-toolbar">
        <span>{language}</span>
        <button onClick={() => void copy()} type="button">
          {copied ? labels.copied : labels.copy}
        </button>
      </div>
      <pre>
        <code>
          <HighlightedCode code={code} language={language} />
        </code>
      </pre>
    </div>
  );
}

function HighlightedCode({ code, language }: { code: string; language: string }) {
  const parts = language === "toml" ? highlightToml(code) : [{ className: "", text: code }];

  return (
    <>
      {parts.map((part, index) => (
        <span className={part.className || undefined} key={`${index}:${part.text}`}>
          {part.text}
        </span>
      ))}
    </>
  );
}

function NestedFieldTable({ allFields, fields }: { allFields: ConfigField[]; fields: ConfigField[] }) {
  const { labels } = useConfigDoc();

  return (
    <div className="qexed-config-table-wrap">
      <table className="qexed-config-table is-nested">
        <thead>
          <tr>
            <th>{labels.path}</th>
            <th>{labels.valueType}</th>
            <th>{labels.defaultValue}</th>
            <th>{labels.currentValue}</th>
            <th>{labels.description}</th>
            <th>{labels.notice}</th>
          </tr>
        </thead>
        <tbody>
          {fields.map((field) => (
            <tr key={field.path}>
              <td>
                <code>{field.path}</code>
              </td>
              <td>
                <code>{field.value_type}</code>
              </td>
              <td>
                <DefaultValue field={field} linkComplex={hasDirectChildren(field, allFields)} />
              </td>
              <td>
                <CurrentValue field={field} linkComplex={hasDirectChildren(field, allFields)} />
              </td>
              <td>
                <RichText text={field.description} />
              </td>
              <td>
                <NoticeList field={field} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function CopySourceButton() {
  const { labels, source } = useConfigDoc();
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    if (!source || typeof navigator === "undefined" || !navigator.clipboard) {
      return;
    }

    await navigator.clipboard.writeText(source);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };

  return (
    <button className="qexed-vscode-button" disabled={!source} onClick={() => void copy()} type="button">
      {copied ? labels.copied : labels.copy}
    </button>
  );
}

function DefaultValue({ field, linkComplex = false }: { field: ConfigField; linkComplex?: boolean }) {
  const { labels } = useConfigDoc();
  if (linkComplex && isComplexField(field)) {
    return <ComplexDetailsLink field={field} />;
  }

  if (!field.default_value) {
    return <span className="qexed-muted">{labels.none}</span>;
  }

  return <ValueDisplay text={field.default_value} />;
}

function CurrentValue({ field, linkComplex = false }: { field: ConfigField; linkComplex?: boolean }) {
  const { labels, values } = useConfigDoc();
  if (linkComplex && isComplexField(field)) {
    return <ComplexDetailsLink field={field} />;
  }

  if (!values) {
    return <span className="qexed-muted">{labels.notLoaded}</span>;
  }

  const result = getValueAtPath(values, field.path);
  if (!result.found) {
    return <span className="qexed-missing">{labels.missing}</span>;
  }

  return <ValueDisplay text={formatValue(result.value)} />;
}

function ValueDisplay({ text }: { text: string }) {
  const isLarge = text.includes("\n") || text.length > 120;
  if (isLarge) {
    return <ExpandableValue code={prettyCode(text)} language={guessLanguage(text)} />;
  }

  return <code>{text}</code>;
}

function ExpandableValue({ code, language }: { code: string; language: string }) {
  const { labels } = useConfigDoc();

  return (
    <details className="qexed-expandable-value">
      <summary>{labels.viewDetails}</summary>
      <CopyableCodeBlock code={code} compact language={language} />
    </details>
  );
}

function ComplexDetailsLink({ field }: { field: ConfigField }) {
  const { labels } = useConfigDoc();
  const text = labels.detailsLink.replace("{path}", field.path);

  return (
    <a className="qexed-details-link" href={`#${detailsId(field.path)}`}>
      {text}
    </a>
  );
}

function NoticeList({ field }: { field: ConfigField }) {
  const { labels } = useConfigDoc();
  const notices = [
    [labels.danger, field.danger],
    [labels.warning, field.warning],
    [labels.pendingDeprecated, field.pending_deprecated],
    [labels.deprecated, field.deprecated],
    [labels.migration, field.migration_notice],
  ].filter((notice): notice is [string, string] => Boolean(notice[1]));

  if (notices.length === 0) {
    return <span className="qexed-muted">{labels.none}</span>;
  }

  return (
    <ul className="qexed-notice-list">
      {notices.map(([label, value]) => (
        <li key={`${label}:${value}`}>
          <strong>{label}:</strong> <RichText text={value} />
        </li>
      ))}
    </ul>
  );
}

function RichText({ text }: { text: string }) {
  const parts = splitFencedCode(text);
  if (parts.length === 1 && parts[0].type === "text") {
    return <>{renderTextLines(parts[0].text)}</>;
  }

  return (
    <div className="qexed-rich-text">
      {parts.map((part, index) =>
        part.type === "code" ? (
          <CopyableCodeBlock code={part.code} key={`code-${index}`} language={part.language} />
        ) : (
          <p key={`text-${index}`}>{renderTextLines(part.text)}</p>
        ),
      )}
    </div>
  );
}

function splitFencedCode(text: string): RichTextPart[] {
  const parts: RichTextPart[] = [];
  const textLines: string[] = [];
  const codeLines: string[] = [];
  let inCode = false;
  let language = "text";

  const flushText = () => {
    const block = textLines.join("\n").trim();
    textLines.length = 0;
    if (block) {
      parts.push({ type: "text", text: block });
    }
  };

  for (const line of text.split(/\r?\n/)) {
    const openFence = line.match(/^```([A-Za-z0-9_-]+)?\s*$/);
    if (!inCode && openFence) {
      flushText();
      inCode = true;
      language = openFence[1] || "text";
      continue;
    }

    if (inCode && line.trim() === "```") {
      parts.push({
        type: "code",
        language,
        code: codeLines.join("\n"),
      });
      codeLines.length = 0;
      inCode = false;
      language = "text";
      continue;
    }

    if (inCode) {
      codeLines.push(line);
    } else {
      textLines.push(line);
    }
  }

  if (inCode) {
    parts.push({
      type: "code",
      language,
      code: codeLines.join("\n"),
    });
  } else {
    flushText();
  }

  return parts.length > 0 ? parts : [{ type: "text", text }];
}

function renderTextLines(text: string): ReactNode[] {
  return text.split(/\r?\n/).flatMap((line, index) =>
    index === 0
      ? [line]
      : [
          <Fragment key={`br-${index}`}>
            <br />
          </Fragment>,
          line,
        ],
  );
}

function useConfigDoc() {
  const context = useContext(ConfigDocContext);
  if (!context) {
    throw new Error("ConfigDoc components must be rendered inside ConfigDocProvider.");
  }

  return context;
}

function getValueAtPath(root: Record<string, unknown>, path: string): LookupResult {
  let current: unknown = root;
  for (const part of path.split(".")) {
    if (!isRecord(current) || !(part in current)) {
      return { found: false };
    }
    current = current[part];
  }

  return { found: true, value: current };
}

function isDirectChildPath(path: string, parentPrefix: string) {
  const suffix = path.startsWith(parentPrefix) ? path.slice(parentPrefix.length) : "";
  return suffix.length > 0 && !suffix.includes(".");
}

function isTopLevelField(field: ConfigField, fields: ConfigField[]) {
  return !fields.some(
    (candidate) => isComplexField(candidate) && field.path.startsWith(`${candidate.path}.`),
  );
}

function isComplexField(field: ConfigField) {
  return field.value_type === "object" || field.value_type === "array";
}

function hasDirectChildren(field: ConfigField, fields: ConfigField[]) {
  const childPrefix = `${field.path}.`;
  return fields.some((candidate) => isDirectChildPath(candidate.path, childPrefix));
}

function detailsId(path: string) {
  return `config-${path.replace(/[^A-Za-z0-9_-]+/g, "-")}`;
}

function formatValue(value: unknown): string {
  if (typeof value === "string") {
    return quoteTomlString(value);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (value === null) {
    return "<null>";
  }

  return formatTomlFragment(value);
}

function prettyCode(text: string): string {
  return text;
}

function guessLanguage(text: string): string {
  const trimmed = text.trim();
  if (trimmed.includes("=") || trimmed.startsWith("[") || trimmed.startsWith("{")) {
    return "toml";
  }

  return "text";
}

function formatTomlFragment(value: unknown): string {
  if (Array.isArray(value)) {
    return formatTomlValue(value);
  }
  if (isRecord(value)) {
    return formatTomlObject(value, "");
  }

  return formatTomlValue(value);
}

function formatTomlObject(value: Record<string, unknown>, prefix: string): string {
  const assignments: string[] = [];
  const sections: string[] = [];

  for (const [key, nestedValue] of Object.entries(value)) {
    if (nestedValue === null) {
      continue;
    }

    if (isRecord(nestedValue)) {
      const section = prefix ? `${prefix}.${key}` : key;
      const body = formatTomlObject(nestedValue, section);
      if (body) {
        sections.push(`[${section}]\n${body}`);
      }
      continue;
    }

    assignments.push(`${formatTomlKey(key)} = ${formatTomlValue(nestedValue)}`);
  }

  return [...assignments, ...sections].join("\n\n") || "{}";
}

function formatTomlValue(value: unknown): string {
  if (typeof value === "string") {
    return quoteTomlString(value);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(formatTomlValue).join(", ")}]`;
  }
  if (isRecord(value)) {
    const entries = Object.entries(value)
      .filter(([, nestedValue]) => nestedValue !== null)
      .map(([key, nestedValue]) => `${formatTomlKey(key)} = ${formatTomlValue(nestedValue)}`);
    return `{ ${entries.join(", ")} }`;
  }

  return "\"<null>\"";
}

function formatTomlKey(key: string): string {
  return /^[A-Za-z0-9_-]+$/.test(key) ? key : quoteTomlString(key);
}

function quoteTomlString(value: string): string {
  return JSON.stringify(value);
}

function highlightToml(code: string): { className: string; text: string }[] {
  const parts: { className: string; text: string }[] = [];
  const push = (className: string, text: string) => {
    if (!text) {
      return;
    }
    const last = parts.at(-1);
    if (last && last.className === className) {
      last.text += text;
    } else {
      parts.push({ className, text });
    }
  };

  for (const line of code.split(/(\r?\n)/)) {
    if (line === "\n" || line === "\r\n") {
      push("", line);
      continue;
    }

    const section = line.match(/^(\s*\[\[?)([^\]]+)(\]?\]\s*)$/);
    if (section) {
      push("qexed-token-punctuation", section[1]);
      push("qexed-token-section", section[2]);
      push("qexed-token-punctuation", section[3]);
      continue;
    }

    const equalIndex = findTopLevelEqual(line);
    if (equalIndex >= 0) {
      push("qexed-token-key", line.slice(0, equalIndex).trimEnd());
      push("", line.slice(line.slice(0, equalIndex).trimEnd().length, equalIndex));
      push("qexed-token-punctuation", "=");
      highlightTomlValue(line.slice(equalIndex + 1), push);
      continue;
    }

    highlightTomlValue(line, push);
  }

  return parts;
}

function highlightTomlValue(value: string, push: (className: string, text: string) => void) {
  const tokenPattern = /("(?:\\.|[^"\\])*"|'[^']*'|\btrue\b|\bfalse\b|[+-]?\b\d+(?:\.\d+)?\b|#.*$|[\[\]{}=,])/g;
  let index = 0;
  for (const match of value.matchAll(tokenPattern)) {
    const token = match[0];
    const start = match.index ?? 0;
    push("", value.slice(index, start));

    if (token.startsWith("\"") || token.startsWith("'")) {
      push("qexed-token-string", token);
    } else if (token === "true" || token === "false") {
      push("qexed-token-boolean", token);
    } else if (token.startsWith("#")) {
      push("qexed-token-comment", token);
    } else if (/^[+-]?\d/.test(token)) {
      push("qexed-token-number", token);
    } else {
      push("qexed-token-punctuation", token);
    }

    index = start + token.length;
  }
  push("", value.slice(index));
}

function parseToml(source: string): Record<string, unknown> {
  const root: Record<string, unknown> = {};
  let tablePath: string[] = [];
  let pending = "";

  for (const rawLine of source.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (!line) {
      continue;
    }

    if (pending) {
      pending = `${pending}\n${line}`;
      if (!isBalanced(pending)) {
        continue;
      }
      applyTomlStatement(root, tablePath, pending);
      pending = "";
      continue;
    }

    if (line.startsWith("[") && line.endsWith("]") && isBalanced(line)) {
      const arrayTable = line.startsWith("[[") && line.endsWith("]]");
      const inner = arrayTable ? line.slice(2, -2).trim() : line.slice(1, -1).trim();
      tablePath = parseKeyPath(inner);
      if (arrayTable) {
        appendArrayTable(root, tablePath);
      } else {
        ensureTable(root, tablePath);
      }
      continue;
    }

    if (!line.includes("=")) {
      throw new Error(`Unsupported TOML line: ${line}`);
    }

    if (!isBalanced(line)) {
      pending = line;
      continue;
    }

    applyTomlStatement(root, tablePath, line);
  }

  if (pending) {
    throw new Error("Unclosed TOML value.");
  }

  return root;
}

function applyTomlStatement(root: Record<string, unknown>, tablePath: string[], statement: string) {
  const equalIndex = findTopLevelEqual(statement);
  if (equalIndex < 0) {
    throw new Error(`Invalid TOML assignment: ${statement}`);
  }

  const key = statement.slice(0, equalIndex).trim();
  const value = statement.slice(equalIndex + 1).trim();
  const target = getTable(root, tablePath);
  setDottedValue(target, parseKeyPath(key), parseTomlValue(value));
}

function parseTomlValue(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  if (trimmed.startsWith("\"\"\"") && trimmed.endsWith("\"\"\"")) {
    return trimmed.slice(3, -3);
  }
  if (trimmed.startsWith("'''") && trimmed.endsWith("'''")) {
    return trimmed.slice(3, -3);
  }
  if (trimmed.startsWith("\"") && trimmed.endsWith("\"")) {
    return JSON.parse(trimmed);
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) {
    return trimmed.slice(1, -1);
  }
  if (trimmed === "true") {
    return true;
  }
  if (trimmed === "false") {
    return false;
  }
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    const inner = trimmed.slice(1, -1).trim();
    return inner ? splitTopLevel(inner, ",").map(parseTomlValue) : [];
  }
  if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
    const table: Record<string, unknown> = {};
    const inner = trimmed.slice(1, -1).trim();
    for (const part of inner ? splitTopLevel(inner, ",") : []) {
      const equalIndex = findTopLevelEqual(part);
      if (equalIndex < 0) {
        throw new Error(`Invalid inline table item: ${part}`);
      }
      const key = parseKeyPath(part.slice(0, equalIndex).trim());
      const nestedValue = parseTomlValue(part.slice(equalIndex + 1).trim());
      setDottedValue(table, key, nestedValue);
    }
    return table;
  }

  const normalizedNumber = trimmed.replaceAll("_", "");
  if (/^[+-]?\d+$/.test(normalizedNumber)) {
    return Number.parseInt(normalizedNumber, 10);
  }
  if (/^[+-]?(\d+\.\d*|\d*\.\d+)([eE][+-]?\d+)?$/.test(normalizedNumber)) {
    return Number.parseFloat(normalizedNumber);
  }

  return trimmed;
}

function stripComment(line: string): string {
  let quote: "\"" | "'" | null = null;
  let escaped = false;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (quote === "\"" && char === "\\" && !escaped) {
      escaped = true;
      continue;
    }
    if ((char === "\"" || char === "'") && !escaped) {
      quote = quote === char ? null : quote ?? char;
    }
    if (char === "#" && quote === null) {
      return line.slice(0, index);
    }
    escaped = false;
  }

  return line;
}

function isBalanced(value: string): boolean {
  let quote: "\"" | "'" | null = null;
  let escaped = false;
  let squareDepth = 0;
  let curlyDepth = 0;

  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (quote === "\"" && char === "\\" && !escaped) {
      escaped = true;
      continue;
    }
    if ((char === "\"" || char === "'") && !escaped) {
      quote = quote === char ? null : quote ?? char;
    } else if (quote === null) {
      if (char === "[") {
        squareDepth += 1;
      } else if (char === "]") {
        squareDepth -= 1;
      } else if (char === "{") {
        curlyDepth += 1;
      } else if (char === "}") {
        curlyDepth -= 1;
      }
    }
    escaped = false;
  }

  return quote === null && squareDepth === 0 && curlyDepth === 0;
}

function findTopLevelEqual(value: string): number {
  let quote: "\"" | "'" | null = null;
  let escaped = false;
  let squareDepth = 0;
  let curlyDepth = 0;

  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (quote === "\"" && char === "\\" && !escaped) {
      escaped = true;
      continue;
    }
    if ((char === "\"" || char === "'") && !escaped) {
      quote = quote === char ? null : quote ?? char;
    } else if (quote === null) {
      if (char === "[") {
        squareDepth += 1;
      } else if (char === "]") {
        squareDepth -= 1;
      } else if (char === "{") {
        curlyDepth += 1;
      } else if (char === "}") {
        curlyDepth -= 1;
      } else if (char === "=" && squareDepth === 0 && curlyDepth === 0) {
        return index;
      }
    }
    escaped = false;
  }

  return -1;
}

function splitTopLevel(value: string, separator: string): string[] {
  const result: string[] = [];
  let quote: "\"" | "'" | null = null;
  let escaped = false;
  let squareDepth = 0;
  let curlyDepth = 0;
  let start = 0;

  for (let index = 0; index < value.length; index += 1) {
    const char = value[index];
    if (quote === "\"" && char === "\\" && !escaped) {
      escaped = true;
      continue;
    }
    if ((char === "\"" || char === "'") && !escaped) {
      quote = quote === char ? null : quote ?? char;
    } else if (quote === null) {
      if (char === "[") {
        squareDepth += 1;
      } else if (char === "]") {
        squareDepth -= 1;
      } else if (char === "{") {
        curlyDepth += 1;
      } else if (char === "}") {
        curlyDepth -= 1;
      } else if (char === separator && squareDepth === 0 && curlyDepth === 0) {
        result.push(value.slice(start, index).trim());
        start = index + 1;
      }
    }
    escaped = false;
  }

  result.push(value.slice(start).trim());
  return result.filter(Boolean);
}

function parseKeyPath(value: string): string[] {
  return splitTopLevel(value, ".").map((part) => {
    const trimmed = part.trim();
    if ((trimmed.startsWith("\"") && trimmed.endsWith("\"")) || (trimmed.startsWith("'") && trimmed.endsWith("'"))) {
      return trimmed.slice(1, -1);
    }
    return trimmed;
  });
}

function getTable(root: Record<string, unknown>, path: string[]): Record<string, unknown> {
  let current = root;
  for (const part of path) {
    const existing = current[part];
    if (Array.isArray(existing)) {
      const last = existing.at(-1);
      if (isRecord(last)) {
        current = last;
        continue;
      }
    }
    if (!isRecord(existing)) {
      current[part] = {};
    }
    current = current[part] as Record<string, unknown>;
  }

  return current;
}

function ensureTable(root: Record<string, unknown>, path: string[]) {
  getTable(root, path);
}

function appendArrayTable(root: Record<string, unknown>, path: string[]) {
  const parent = getTable(root, path.slice(0, -1));
  const key = path.at(-1);
  if (!key) {
    return;
  }

  const existing = parent[key];
  if (!Array.isArray(existing)) {
    parent[key] = [];
  }
  (parent[key] as Record<string, unknown>[]).push({});
}

function setDottedValue(target: Record<string, unknown>, path: string[], value: unknown) {
  let current = target;
  for (const part of path.slice(0, -1)) {
    if (!isRecord(current[part])) {
      current[part] = {};
    }
    current = current[part] as Record<string, unknown>;
  }

  const key = path.at(-1);
  if (key) {
    current[key] = value;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function ConfigDocStyles() {
  return (
    <style>{`
      .qexed-vscode-shell {
        display: grid;
        grid-template-columns: 44px minmax(190px, 260px) minmax(0, 1fr);
        overflow: hidden;
        margin: 24px 0;
        border: 1px solid #2b2b2b;
        border-radius: 8px;
        background: #1e1e1e;
        color: #d4d4d4;
        box-shadow: 0 18px 45px rgba(0, 0, 0, 0.24);
      }
      .qexed-vscode-activity {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 14px;
        padding-top: 16px;
        background: #333333;
      }
      .qexed-vscode-activity span {
        width: 16px;
        height: 16px;
        border-radius: 4px;
        border: 1px solid #858585;
      }
      .qexed-vscode-sidebar {
        min-width: 0;
        overflow: hidden;
        border-right: 1px solid #252526;
        background: #252526;
        color: #cccccc;
      }
      .qexed-vscode-sidebar-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        padding-right: 10px;
      }
      .qexed-vscode-sidebar-title {
        display: flex;
        align-items: center;
        gap: 6px;
        min-width: 0;
        padding: 12px 10px;
        color: #ffffff;
        font-size: 12px;
        font-weight: 700;
        letter-spacing: 0;
      }
      .qexed-vscode-sidebar-actions {
        display: flex;
        gap: 10px;
        flex: 0 0 auto;
      }
      .qexed-vscode-sidebar-actions span {
        width: 12px;
        height: 12px;
        border: 1px solid #b8b8b8;
        border-radius: 3px;
      }
      .qexed-vscode-tree {
        max-height: 440px;
        overflow: auto;
        padding-bottom: 10px;
        font: 13px/1.45 "Segoe UI", system-ui, sans-serif;
      }
      .qexed-vscode-tree-group {
        margin-left: 18px;
      }
      .qexed-vscode-tree-folder,
      .qexed-vscode-tree-file {
        display: flex;
        align-items: center;
        gap: 7px;
        overflow: hidden;
        min-height: 28px;
        padding: 0 8px;
        color: #d4d4d4;
        text-decoration: none;
      }
      .qexed-vscode-tree-file:hover {
        background: #2a2d2e;
        color: #ffffff;
      }
      .qexed-vscode-tree-file.is-active {
        background: #37373d;
        color: #ffffff;
      }
      .qexed-vscode-tree-folder span:last-child,
      .qexed-vscode-tree-file span:last-child {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .qexed-vscode-tree-arrow {
        width: 8px;
        height: 8px;
        border-right: 1px solid #cccccc;
        border-bottom: 1px solid #cccccc;
        transform: rotate(-45deg);
        flex: 0 0 auto;
      }
      .qexed-vscode-tree-arrow.is-open {
        transform: rotate(45deg);
      }
      .qexed-vscode-folder-icon {
        width: 14px;
        height: 11px;
        border: 1px solid #c5c5c5;
        border-top-width: 3px;
        border-radius: 2px;
        flex: 0 0 auto;
      }
      .qexed-vscode-file-icon {
        position: relative;
        width: 12px;
        height: 15px;
        border: 1px solid #c5c5c5;
        border-radius: 1px;
        flex: 0 0 auto;
      }
      .qexed-vscode-file-icon::after {
        content: "";
        position: absolute;
        top: -1px;
        right: -1px;
        width: 5px;
        height: 5px;
        border-left: 1px solid #c5c5c5;
        border-bottom: 1px solid #c5c5c5;
        background: #252526;
      }
      .qexed-vscode-main {
        min-width: 0;
      }
      .qexed-vscode-titlebar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        padding: 14px 16px;
        border-bottom: 1px solid #2d2d30;
      }
      .qexed-vscode-title {
        color: #ffffff;
        font-weight: 600;
      }
      .qexed-vscode-subtitle {
        margin-top: 4px;
        color: #9cdcfe;
        font-size: 13px;
      }
      .qexed-vscode-actions {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }
      .qexed-vscode-button,
      .qexed-code-toolbar button {
        border: 1px solid #3c3c3c;
        border-radius: 4px;
        background: #0e639c;
        color: #ffffff;
        cursor: pointer;
        font: 12px/1.2 inherit;
        padding: 7px 10px;
      }
      .qexed-vscode-button:disabled {
        cursor: not-allowed;
        opacity: 0.48;
      }
      .qexed-vscode-button input {
        display: none;
      }
      .qexed-vscode-tabbar {
        height: 36px;
        border-bottom: 1px solid #252526;
        background: #2d2d2d;
      }
      .qexed-vscode-tab {
        display: inline-flex;
        align-items: center;
        height: 36px;
        padding: 0 14px;
        border-right: 1px solid #252526;
        background: #1e1e1e;
        color: #ffffff;
        font: 13px/1 Consolas, "SFMono-Regular", Menlo, monospace;
      }
      .qexed-vscode-editor {
        display: grid;
        grid-template-columns: 56px minmax(0, 1fr);
        height: clamp(320px, 52vh, 560px);
        min-height: 0;
        overflow: hidden;
        background: #1e1e1e;
        font: 13px/1.55 Consolas, "SFMono-Regular", Menlo, monospace;
      }
      .qexed-vscode-lines {
        height: 100%;
        margin: 0;
        padding: 12px 12px 12px 0;
        color: #858585;
        overflow: hidden;
        text-align: right;
        user-select: none;
      }
      .qexed-vscode-textarea {
        width: 100%;
        min-width: 0;
        height: 100%;
        min-height: 0;
        padding: 12px 14px;
        border: 0;
        outline: none;
        overflow: auto;
        resize: none;
        background: transparent;
        color: #d4d4d4;
        caret-color: #ffffff;
        font: inherit;
      }
      .qexed-vscode-textarea::placeholder {
        color: #6a9955;
      }
      .qexed-vscode-status {
        padding: 7px 12px;
        background: #007acc;
        color: #ffffff;
        font-size: 12px;
      }
      .qexed-vscode-status.is-error {
        background: #a1260d;
      }
      .qexed-vscode-status.is-ok {
        background: #16825d;
      }
      .qexed-config-table-wrap {
        overflow-x: auto;
        margin: 18px 0;
      }
      .qexed-config-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 14px;
      }
      .qexed-config-table th,
      .qexed-config-table td {
        vertical-align: top;
        border: 1px solid rgba(127, 127, 127, 0.28);
        padding: 10px 12px;
      }
      .qexed-config-table th {
        background: rgba(30, 30, 30, 0.08);
        text-align: left;
      }
      .qexed-config-table code,
      .qexed-complex-details code {
        border-radius: 4px;
        padding: 2px 5px;
        background: rgba(127, 127, 127, 0.14);
      }
      .qexed-muted {
        color: #7a7a7a;
      }
      .qexed-missing {
        color: #c586c0;
      }
      .qexed-notice-list {
        margin: 0;
        padding-left: 18px;
      }
      .qexed-rich-text {
        display: grid;
        gap: 10px;
      }
      .qexed-rich-text p {
        margin: 0;
      }
      .qexed-code-block {
        overflow: hidden;
        border: 1px solid #30363d;
        border-radius: 6px;
        background: #0d1117;
      }
      .qexed-code-block.is-compact {
        max-width: 460px;
      }
      .qexed-code-toolbar {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 12px;
        padding: 7px 9px;
        border-bottom: 1px solid #30363d;
        color: #9cdcfe;
        font-size: 12px;
      }
      .qexed-code-toolbar button {
        padding: 5px 8px;
      }
      .qexed-code-block pre {
        overflow: auto;
        margin: 0;
        padding: 10px 12px;
        color: #d4d4d4;
        font: 12px/1.55 Consolas, "SFMono-Regular", Menlo, monospace;
      }
      .qexed-token-key {
        color: #9cdcfe;
      }
      .qexed-token-section {
        color: #4ec9b0;
      }
      .qexed-token-string {
        color: #ce9178;
      }
      .qexed-token-number {
        color: #b5cea8;
      }
      .qexed-token-boolean {
        color: #569cd6;
      }
      .qexed-token-comment {
        color: #6a9955;
      }
      .qexed-token-punctuation {
        color: #d4d4d4;
      }
      .qexed-expandable-value summary {
        display: inline-flex;
        align-items: center;
        min-height: 28px;
        color: #0e639c;
        cursor: pointer;
        font-weight: 600;
      }
      .qexed-expandable-value[open] summary {
        margin-bottom: 8px;
      }
      .qexed-details-link {
        color: #0e639c;
        font-weight: 600;
        text-decoration: none;
      }
      .qexed-details-link:hover {
        text-decoration: underline;
      }
      .qexed-complex-field {
        margin-top: 22px;
      }
      .qexed-complex-field dl {
        display: grid;
        gap: 8px;
      }
      .qexed-complex-field dl > div {
        display: grid;
        grid-template-columns: minmax(120px, 180px) minmax(0, 1fr);
        gap: 12px;
      }
      .qexed-complex-field dt {
        color: #666666;
        font-weight: 600;
      }
      @media (max-width: 760px) {
        .qexed-vscode-shell {
          grid-template-columns: 36px minmax(0, 1fr);
        }
        .qexed-vscode-sidebar {
          display: none;
        }
        .qexed-vscode-titlebar {
          align-items: stretch;
          flex-direction: column;
        }
        .qexed-vscode-editor {
          grid-template-columns: 44px minmax(0, 1fr);
        }
        .qexed-complex-field dl > div {
          grid-template-columns: 1fr;
        }
      }
    `}</style>
  );
}
"###;
rust_i18n::i18n!("./locales");
#[derive(Debug, Parser)]
#[command(name = "qexed_config_to_mdx")]
#[command(about = "Generate Qexed AutoDoc config documentation packages.")]
struct Args {
    /// 输出根目录。实际内容会生成在 <out>/<commit>/ 下。
    #[arg(long, default_value = "docs/config")]
    out: PathBuf,

    /// 文档语言，可重复传入；默认生成 zh-CN 与 en。
    #[arg(long = "lang")]
    langs: Vec<String>,

    /// 输出格式，可重复传入；默认 all。
    #[arg(long = "format", value_enum)]
    formats: Vec<OutputFormat>,

    /// 覆盖输出目录使用的 commit hash。
    #[arg(long)]
    commit: Option<String>,

    /// 直接写入 Next.js app router 插件 API AutoDoc 页面目录。
    ///
    /// 目标目录应为官网仓库的 app/docs/plugin-api；生成器会写入 page.mdx 与 en/page.mdx。
    #[arg(long)]
    plugin_docs_out: Option<PathBuf>,

    /// 直接写入 Next.js app router Placeholder AutoDoc 页面目录。
    ///
    /// 目标目录应为官网仓库的 app/docs/placeholders；生成器会写入版本化页面与 versions.json。
    #[arg(long)]
    placeholder_docs_out: Option<PathBuf>,

    /// 只生成 Placeholder 文档，跳过配置文档收集。
    #[arg(long)]
    placeholders_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    All,
    Markdown,
    Mdx,
    NextApp,
    NextPages,
    Mkdocs,
    Mdbook,
    Json,
}

impl OutputFormat {
    fn name(&self) -> String {
        match self {
            Self::All => "all",
            Self::Markdown => "markdown",
            Self::Mdx => "mdx",
            Self::NextApp => "next-app",
            Self::NextPages => "next-pages",
            Self::Mkdocs => "mkdocs",
            Self::Mdbook => "mdbook",
            Self::Json => "json",
        }
        .to_string()
    }
}

#[derive(Debug, Serialize)]
struct Manifest {
    commit: String,
    generator: &'static str,
    generated_formats: Vec<String>,
    languages: Vec<String>,
    apps: Vec<AppManifest>,
    asset_logo: String,
}

#[derive(Debug, Serialize)]
struct AppManifest {
    name: String,
    config_file: String,
    field_count: usize,
}

#[derive(Debug, Serialize)]
struct DocBundle {
    commit: String,
    languages: Vec<LanguageDocs>,
}

#[derive(Debug, Serialize)]
struct LanguageDocs {
    lang: String,
    apps: Vec<DocApp>,
}

#[derive(Debug, Clone, Serialize)]
struct DocApp {
    name: String,
    config_file: String,
    config_path: String,
    description: String,
    root_path: Option<String>,
    fields: Vec<DocField>,
}

#[derive(Debug, Serialize)]
struct ConfigDocFile {
    name: String,
    #[serde(rename = "configPath")]
    config_path: String,
    route: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
struct DocField {
    path: String,
    description: String,
    value_type: String,
    default_value: Option<String>,
    warning: Option<String>,
    danger: Option<String>,
    pending_deprecated: Option<String>,
    deprecated: Option<String>,
    migration_notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginCallKind {
    Event,
    Query,
    Hybrid,
}

#[derive(Debug, Clone, Copy)]
struct PluginEventDoc {
    event: PluginEvent,
    kind: PluginCallKind,
    payload: Option<&'static str>,
    response: Option<&'static str>,
    zh: &'static str,
    en: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct PluginStructDoc {
    name: &'static str,
    zh: &'static str,
    en: &'static str,
    fields: &'static [PluginFieldDoc],
}

#[derive(Debug, Clone, Copy)]
struct PluginFieldDoc {
    name: &'static str,
    ty: &'static str,
    default: &'static str,
    zh: &'static str,
    en: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct PluginHostApiDoc {
    name: &'static str,
    signature: &'static str,
    wrapper: &'static str,
    zh: &'static str,
    en: &'static str,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaceholderDocsVersion {
    channel: String,
    id: String,
    label: String,
    source_ref: String,
    source_sha: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_date: Option<String>,
}

const PLUGIN_EVENT_DOCS: &[PluginEventDoc] = &[
    PluginEventDoc {
        event: PluginEvent::Init,
        kind: PluginCallKind::Event,
        payload: None,
        response: None,
        zh: "插件加载并完成实例化后调用一次。适合初始化配置、注册宿主资源或写入启动日志。",
        en: "Called once after a plugin is loaded and instantiated. Use it for configuration bootstrap, host resource registration, or startup logs.",
    },
    PluginEventDoc {
        event: PluginEvent::PlayerJoin,
        kind: PluginCallKind::Event,
        payload: Some("PlayerPayload"),
        response: None,
        zh: "玩家进入服务器时触发。",
        en: "Emitted when a player joins the server.",
    },
    PluginEventDoc {
        event: PluginEvent::PlayerLeave,
        kind: PluginCallKind::Event,
        payload: Some("PlayerPayload"),
        response: None,
        zh: "玩家离开服务器时触发。",
        en: "Emitted when a player leaves the server.",
    },
    PluginEventDoc {
        event: PluginEvent::ChunkLoad,
        kind: PluginCallKind::Event,
        payload: Some("ChunkPayload"),
        response: None,
        zh: "区块进入服务端加载视野时触发。",
        en: "Emitted when a chunk enters the server-side loaded view.",
    },
    PluginEventDoc {
        event: PluginEvent::ChunkUnload,
        kind: PluginCallKind::Event,
        payload: Some("ChunkPayload"),
        response: None,
        zh: "区块从服务端加载视野卸载时触发。",
        en: "Emitted when a chunk leaves the server-side loaded view.",
    },
    PluginEventDoc {
        event: PluginEvent::ConfigReload,
        kind: PluginCallKind::Event,
        payload: Some("ConfigReloadPayload"),
        response: None,
        zh: "配置加载或重载后触发，payload 中包含配置路径。",
        en: "Emitted after a config file is loaded or reloaded; the payload contains the config path.",
    },
    PluginEventDoc {
        event: PluginEvent::LanguageChange,
        kind: PluginCallKind::Event,
        payload: Some("LanguagePayload"),
        response: None,
        zh: "服务器运行语言变化后触发。",
        en: "Emitted when the runtime language changes.",
    },
    PluginEventDoc {
        event: PluginEvent::MiningSpeed,
        kind: PluginCallKind::Query,
        payload: Some("MiningSpeedQuery"),
        response: Some("MiningSpeedResponse"),
        zh: "允许插件按方块、工具和附魔修改挖掘速度。",
        en: "Lets plugins adjust mining speed by block, tool, and enchantments.",
    },
    PluginEventDoc {
        event: PluginEvent::BlockDrops,
        kind: PluginCallKind::Query,
        payload: Some("BlockDropQuery"),
        response: Some("BlockDropResponse"),
        zh: "允许插件替换或追加方块掉落物。",
        en: "Lets plugins replace or append block drops.",
    },
    PluginEventDoc {
        event: PluginEvent::Commands,
        kind: PluginCallKind::Query,
        payload: None,
        response: Some("PluginCommandDefinition"),
        zh: "收集插件提供的玩家命令定义。",
        en: "Collects player command definitions provided by plugins.",
    },
    PluginEventDoc {
        event: PluginEvent::CommandExecute,
        kind: PluginCallKind::Query,
        payload: Some("PluginCommandQuery"),
        response: Some("PluginCommandResponse"),
        zh: "玩家执行插件命令时调用，返回要执行的玩家动作。",
        en: "Called when a player runs a plugin command and returns player actions.",
    },
    PluginEventDoc {
        event: PluginEvent::NpcMutations,
        kind: PluginCallKind::Query,
        payload: Some("NpcMutationQuery"),
        response: Some("NpcMutationResponse"),
        zh: "允许插件动态新增、更新或移除 NPC。",
        en: "Lets plugins dynamically add, update, or remove NPCs.",
    },
    PluginEventDoc {
        event: PluginEvent::NpcInteract,
        kind: PluginCallKind::Hybrid,
        payload: Some("NpcInteractPayload"),
        response: Some("PluginCommandResponse"),
        zh: "玩家与 NPC 交互时触发，可按主手、副手和攻击事件返回动作。",
        en: "Triggered when a player interacts with an NPC; it can return actions for main-hand, off-hand, and attack events.",
    },
    PluginEventDoc {
        event: PluginEvent::ProxyConnectResult,
        kind: PluginCallKind::Event,
        payload: Some("ProxyConnectResultPayload"),
        response: None,
        zh: "跨服代理连接完成或失败后回调。",
        en: "Called after a proxy transfer succeeds or fails.",
    },
    PluginEventDoc {
        event: PluginEvent::Placeholders,
        kind: PluginCallKind::Query,
        payload: Some("PlaceholderQuery"),
        response: Some("PlaceholderResponse"),
        zh: "解析服务器文本中的插件占位符。",
        en: "Resolves plugin placeholders in server text.",
    },
    PluginEventDoc {
        event: PluginEvent::PlayerBlockStep,
        kind: PluginCallKind::Query,
        payload: Some("BlockStepPayload"),
        response: Some("PluginCommandResponse"),
        zh: "玩家踩到方块时调用，适合跳板、区域触发器等逻辑。",
        en: "Called when a player steps on a block; useful for jump pads, region triggers, and similar logic.",
    },
    PluginEventDoc {
        event: PluginEvent::PlayerMove,
        kind: PluginCallKind::Query,
        payload: Some("PlayerMovePayload"),
        response: Some("PluginCommandResponse"),
        zh: "玩家位置发生变化时调用。",
        en: "Called when a player's position changes.",
    },
    PluginEventDoc {
        event: PluginEvent::PlayerInput,
        kind: PluginCallKind::Query,
        payload: Some("PlayerInputPayload"),
        response: Some("PluginCommandResponse"),
        zh: "玩家输入状态变化时调用，适合二段跳等玩法。",
        en: "Called when a player's input state changes; useful for mechanics such as double jump.",
    },
    PluginEventDoc {
        event: PluginEvent::ClickDetected,
        kind: PluginCallKind::Event,
        payload: Some("ClickDetectedPayload"),
        response: None,
        zh: "连点检测命中后通知插件。",
        en: "Notifies plugins when click detection reports a burst.",
    },
    PluginEventDoc {
        event: PluginEvent::Pathfinding,
        kind: PluginCallKind::Query,
        payload: Some("PathfindingQuery"),
        response: Some("PathfindingResponse"),
        zh: "预留的寻路查询事件；当前推荐使用宿主导入 qexed::pathfinding_find。",
        en: "Reserved pathfinding query event; currently prefer the qexed::pathfinding_find host import.",
    },
    PluginEventDoc {
        event: PluginEvent::CustomEntities,
        kind: PluginCallKind::Query,
        payload: None,
        response: Some("CustomEntityRegistryResponse"),
        zh: "注册插件自定义实体类型及其默认 Minecraft 实体类型和 AI 标识。",
        en: "Registers plugin-defined entity types with their backing Minecraft entity type and AI identifier.",
    },
    PluginEventDoc {
        event: PluginEvent::EntityAiTick,
        kind: PluginCallKind::Query,
        payload: Some("EntityAiTickQuery"),
        response: Some("EntityAiTickResponse"),
        zh: "实体 AI tick 时调用，插件可返回移动、看向目标或移除实体等操作。",
        en: "Called during an entity AI tick; plugins may return movement, look-at, or removal operations.",
    },
];

const PLUGIN_STRUCT_DOCS: &[PluginStructDoc] = &[
    PluginStructDoc {
        name: "PlayerPayload",
        zh: "玩家事件 payload。",
        en: "Player event payload.",
        fields: &[
            field("uuid", "String", "", "玩家 UUID。", "Player UUID."),
            field("username", "String", "", "玩家名。", "Username."),
            field(
                "entity_id",
                "i32",
                "",
                "服务端实体 ID。",
                "Server entity ID.",
            ),
            field(
                "language",
                "String",
                "\"\"",
                "玩家语言。",
                "Player language.",
            ),
            field(
                "dimension",
                "String",
                "\"minecraft:overworld\"",
                "玩家所在维度。",
                "Player dimension.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PlayerPayloadOwned",
        zh: "拥有所有字段的玩家 payload，用于需要嵌套保存的查询结构。",
        en: "Owned player payload used by query structures that need nested player data.",
        fields: &[
            field("uuid", "String", "", "玩家 UUID。", "Player UUID."),
            field("username", "String", "", "玩家名。", "Username."),
            field(
                "entity_id",
                "i32",
                "",
                "服务端实体 ID。",
                "Server entity ID.",
            ),
            field(
                "language",
                "String",
                "\"\"",
                "玩家语言。",
                "Player language.",
            ),
            field(
                "dimension",
                "String",
                "\"minecraft:overworld\"",
                "玩家所在维度。",
                "Player dimension.",
            ),
        ],
    },
    PluginStructDoc {
        name: "ChunkPayload",
        zh: "区块加载/卸载事件 payload。",
        en: "Chunk load/unload event payload.",
        fields: &[
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field("chunk_x", "i32", "", "区块 X 坐标。", "Chunk X coordinate."),
            field("chunk_z", "i32", "", "区块 Z 坐标。", "Chunk Z coordinate."),
        ],
    },
    PluginStructDoc {
        name: "ConfigReloadPayload",
        zh: "配置重载事件 payload。",
        en: "Config reload event payload.",
        fields: &[field("path", "String", "", "配置路径。", "Config path.")],
    },
    PluginStructDoc {
        name: "LanguagePayload",
        zh: "语言变化事件 payload。",
        en: "Language change event payload.",
        fields: &[field(
            "language",
            "String",
            "",
            "当前语言。",
            "Current language.",
        )],
    },
    PluginStructDoc {
        name: "ItemEnchantment",
        zh: "原版附魔条目。",
        en: "Vanilla enchantment entry.",
        fields: &[
            field(
                "id",
                "String",
                "",
                "附魔资源键。",
                "Enchantment resource key.",
            ),
            field("level", "i32", "", "附魔等级。", "Enchantment level."),
        ],
    },
    PluginStructDoc {
        name: "PluginEnchantment",
        zh: "插件自定义附魔条目。",
        en: "Plugin-defined enchantment entry.",
        fields: &[
            field(
                "id",
                "String",
                "",
                "自定义附魔 ID。",
                "Custom enchantment ID.",
            ),
            field("level", "i32", "", "附魔等级。", "Enchantment level."),
        ],
    },
    PluginStructDoc {
        name: "MiningSpeedQuery",
        zh: "挖掘速度查询。",
        en: "Mining speed query.",
        fields: &[
            field("block_state", "i32", "", "方块状态 ID。", "Block state ID."),
            field(
                "block_name",
                "String",
                "",
                "方块资源键。",
                "Block resource key.",
            ),
            field(
                "item_id",
                "Option<i32>",
                "None",
                "工具物品 ID。",
                "Tool item ID.",
            ),
            field(
                "enchantments",
                "Vec<ItemEnchantment>",
                "[]",
                "原版附魔。",
                "Vanilla enchantments.",
            ),
            field(
                "plugin_enchantments",
                "Vec<PluginEnchantment>",
                "[]",
                "插件附魔。",
                "Plugin enchantments.",
            ),
            field(
                "speed",
                "f32",
                "",
                "当前挖掘速度。",
                "Current mining speed.",
            ),
        ],
    },
    PluginStructDoc {
        name: "MiningSpeedResponse",
        zh: "挖掘速度响应。",
        en: "Mining speed response.",
        fields: &[
            field(
                "speed",
                "Option<f32>",
                "None",
                "直接覆盖速度。",
                "Override speed.",
            ),
            field(
                "multiplier",
                "Option<f32>",
                "None",
                "乘法修正。",
                "Multiplicative modifier.",
            ),
            field(
                "add",
                "Option<f32>",
                "None",
                "加法修正。",
                "Additive modifier.",
            ),
        ],
    },
    PluginStructDoc {
        name: "BlockDropPosition",
        zh: "方块坐标。",
        en: "Block position.",
        fields: &[
            field("x", "i32", "", "X 坐标。", "X coordinate."),
            field("y", "i32", "", "Y 坐标。", "Y coordinate."),
            field("z", "i32", "", "Z 坐标。", "Z coordinate."),
        ],
    },
    PluginStructDoc {
        name: "BlockDropQuery",
        zh: "方块掉落查询。",
        en: "Block drop query.",
        fields: &[
            field("block_state", "i32", "", "方块状态 ID。", "Block state ID."),
            field(
                "block_name",
                "String",
                "",
                "方块资源键。",
                "Block resource key.",
            ),
            field(
                "position",
                "BlockDropPosition",
                "",
                "方块位置。",
                "Block position.",
            ),
            field(
                "tool_item_id",
                "Option<i32>",
                "None",
                "工具物品 ID。",
                "Tool item ID.",
            ),
            field(
                "enchantments",
                "Vec<ItemEnchantment>",
                "[]",
                "原版附魔。",
                "Vanilla enchantments.",
            ),
            field(
                "plugin_enchantments",
                "Vec<PluginEnchantment>",
                "[]",
                "插件附魔。",
                "Plugin enchantments.",
            ),
            field(
                "default_item_id",
                "Option<i32>",
                "None",
                "默认掉落物品 ID。",
                "Default drop item ID.",
            ),
        ],
    },
    PluginStructDoc {
        name: "BlockDropResponse",
        zh: "方块掉落响应。",
        en: "Block drop response.",
        fields: &[
            field(
                "replace",
                "bool",
                "false",
                "是否替换默认掉落。",
                "Whether to replace default drops.",
            ),
            field(
                "items",
                "Vec<BlockDropItem>",
                "[]",
                "要追加或替换的物品。",
                "Items to append or replace with.",
            ),
        ],
    },
    PluginStructDoc {
        name: "BlockDropItem",
        zh: "掉落物条目。",
        en: "Drop item entry.",
        fields: &[
            field("item_id", "i32", "", "物品 ID。", "Item ID."),
            field("count", "i32", "1", "数量。", "Count."),
        ],
    },
    PluginStructDoc {
        name: "BlockStepPosition",
        zh: "玩家踩踏方块坐标。",
        en: "Block position stepped on by a player.",
        fields: &[
            field("x", "i32", "", "X 坐标。", "X coordinate."),
            field("y", "i32", "", "Y 坐标。", "Y coordinate."),
            field("z", "i32", "", "Z 坐标。", "Z coordinate."),
        ],
    },
    PluginStructDoc {
        name: "PlayerPositionPayload",
        zh: "玩家或实体位置。",
        en: "Player or entity position.",
        fields: &[
            field("x", "f64", "", "X 坐标。", "X coordinate."),
            field("y", "f64", "", "Y 坐标。", "Y coordinate."),
            field("z", "f64", "", "Z 坐标。", "Z coordinate."),
            field("yaw", "f32", "", "水平朝向。", "Yaw."),
            field("pitch", "f32", "", "俯仰角。", "Pitch."),
            field(
                "on_ground",
                "bool",
                "",
                "是否在地面上。",
                "Whether the entity is on the ground.",
            ),
        ],
    },
    PluginStructDoc {
        name: "BlockStepPayload",
        zh: "玩家踩踏方块查询。",
        en: "Player block-step query.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field("block_state", "i32", "", "方块状态 ID。", "Block state ID."),
            field(
                "block_name",
                "String",
                "",
                "方块资源键。",
                "Block resource key.",
            ),
            field(
                "position",
                "BlockStepPosition",
                "",
                "方块位置。",
                "Block position.",
            ),
            field(
                "player_position",
                "PlayerPositionPayload",
                "",
                "玩家位置。",
                "Player position.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PlayerMovePayload",
        zh: "玩家移动查询。",
        en: "Player movement query.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field(
                "previous_position",
                "PlayerPositionPayload",
                "",
                "移动前位置。",
                "Previous position.",
            ),
            field(
                "position",
                "PlayerPositionPayload",
                "",
                "当前位置。",
                "Current position.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PlayerInputState",
        zh: "玩家输入状态。",
        en: "Player input state.",
        fields: &[
            field("forward", "bool", "", "前进键。", "Forward key."),
            field("backward", "bool", "", "后退键。", "Backward key."),
            field("left", "bool", "", "左移键。", "Left key."),
            field("right", "bool", "", "右移键。", "Right key."),
            field("jump", "bool", "", "跳跃键。", "Jump key."),
            field("shift", "bool", "", "潜行键。", "Sneak key."),
            field("sprint", "bool", "", "疾跑键。", "Sprint key."),
        ],
    },
    PluginStructDoc {
        name: "PlayerInputPayload",
        zh: "玩家输入查询。",
        en: "Player input query.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field(
                "position",
                "PlayerPositionPayload",
                "",
                "玩家位置。",
                "Player position.",
            ),
            field(
                "previous_input",
                "PlayerInputState",
                "",
                "上一输入状态。",
                "Previous input state.",
            ),
            field(
                "input",
                "PlayerInputState",
                "",
                "当前输入状态。",
                "Current input state.",
            ),
        ],
    },
    PluginStructDoc {
        name: "ClickDetectedPayload",
        zh: "连点检测事件 payload。",
        en: "Click detection event payload.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field(
                "position",
                "PlayerPositionPayload",
                "",
                "玩家位置。",
                "Player position.",
            ),
            field("action", "String", "", "点击动作。", "Click action."),
            field(
                "clicks",
                "u32",
                "",
                "窗口内点击次数。",
                "Clicks inside the window.",
            ),
            field(
                "window_ms",
                "u64",
                "",
                "统计窗口毫秒数。",
                "Counting window in milliseconds.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PathfindingQuery",
        zh: "寻路查询。",
        en: "Pathfinding query.",
        fields: &[
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field(
                "start",
                "BlockDropPosition",
                "",
                "起点。",
                "Start position.",
            ),
            field("goal", "BlockDropPosition", "", "终点。", "Goal position."),
            field(
                "max_nodes",
                "usize",
                "0",
                "最大搜索节点数。",
                "Maximum searched node count.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PathfindingResponse",
        zh: "寻路响应。",
        en: "Pathfinding response.",
        fields: &[
            field(
                "found",
                "bool",
                "false",
                "是否找到路径。",
                "Whether a path was found.",
            ),
            field(
                "path",
                "Vec<BlockDropPosition>",
                "[]",
                "路径节点。",
                "Path nodes.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PluginCommandDefinition",
        zh: "插件命令定义。",
        en: "Plugin command definition.",
        fields: &[
            field(
                "name",
                "String",
                "",
                "命令名，不包含斜杠。",
                "Command name without slash.",
            ),
            field(
                "description_key",
                "String",
                "\"\"",
                "命令说明翻译键。",
                "Command description translation key.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PluginCommandQuery",
        zh: "插件命令执行查询。",
        en: "Plugin command execution query.",
        fields: &[
            field("command", "String", "", "命令名。", "Command name."),
            field(
                "argument",
                "String",
                "",
                "原始参数文本。",
                "Raw argument text.",
            ),
            field(
                "player",
                "PlayerPayloadOwned",
                "",
                "执行命令的玩家。",
                "Player executing the command.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PluginCommandResponse",
        zh: "插件动作响应。",
        en: "Plugin action response.",
        fields: &[
            field(
                "handled",
                "bool",
                "false",
                "插件是否处理了该调用。",
                "Whether the plugin handled the call.",
            ),
            field(
                "actions",
                "Vec<PlayerAction>",
                "[]",
                "服务端要执行的玩家动作。",
                "Player actions for the server to run.",
            ),
        ],
    },
    PluginStructDoc {
        name: "ProxyConnectResultPayload",
        zh: "跨服连接结果事件 payload。",
        en: "Proxy connection result event payload.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "target_server",
                "String",
                "",
                "目标子服。",
                "Target server.",
            ),
            field(
                "current_server",
                "String",
                "\"\"",
                "当前子服。",
                "Current server.",
            ),
            field(
                "proxy_protocol",
                "String",
                "\"\"",
                "代理协议。",
                "Proxy protocol.",
            ),
            field("status_code", "i32", "", "状态码。", "Status code."),
            field("status", "String", "", "状态文本。", "Status text."),
            field(
                "success",
                "bool",
                "",
                "是否成功。",
                "Whether the transfer succeeded.",
            ),
            field(
                "message",
                "String",
                "\"\"",
                "附加消息。",
                "Additional message.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PlaceholderQuery",
        zh: "占位符解析查询。",
        en: "Placeholder resolution query.",
        fields: &[
            field(
                "player",
                "Option<PlayerPayloadOwned>",
                "None",
                "可选玩家上下文。",
                "Optional player context.",
            ),
            field("text", "String", "", "待解析文本。", "Text to resolve."),
            field(
                "context",
                "Vec<PlaceholderContext>",
                "[]",
                "额外上下文。",
                "Additional context.",
            ),
        ],
    },
    PluginStructDoc {
        name: "PlaceholderContext",
        zh: "占位符上下文键值对。",
        en: "Placeholder context key/value pair.",
        fields: &[
            field("key", "String", "", "上下文键。", "Context key."),
            field("value", "String", "", "上下文值。", "Context value."),
        ],
    },
    PluginStructDoc {
        name: "PlaceholderResponse",
        zh: "占位符响应。",
        en: "Placeholder response.",
        fields: &[field(
            "replacements",
            "Vec<PlaceholderReplacement>",
            "[]",
            "替换项。",
            "Replacements.",
        )],
    },
    PluginStructDoc {
        name: "PlaceholderReplacement",
        zh: "占位符替换项。",
        en: "Placeholder replacement.",
        fields: &[
            field("key", "String", "", "占位符键。", "Placeholder key."),
            field("value", "String", "", "替换值。", "Replacement value."),
        ],
    },
    PluginStructDoc {
        name: "NpcInteractPayload",
        zh: "NPC 交互 payload。",
        en: "NPC interaction payload.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "entity",
                "NpcEntityPayload",
                "",
                "NPC 实体。",
                "NPC entity.",
            ),
            field("action", "String", "", "动作类型。", "Action type."),
            field("hand", "String", "\"\"", "交互手。", "Interaction hand."),
            field(
                "configured_event",
                "String",
                "\"\"",
                "配置中绑定的事件名。",
                "Event name configured for the NPC.",
            ),
        ],
    },
    PluginStructDoc {
        name: "NpcEntityPayload",
        zh: "NPC 实体 payload。",
        en: "NPC entity payload.",
        fields: &[
            field("key", "String", "", "NPC 配置键。", "NPC config key."),
            field(
                "entity_id",
                "i32",
                "",
                "服务端实体 ID。",
                "Server entity ID.",
            ),
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field("x", "f64", "", "X 坐标。", "X coordinate."),
            field("y", "f64", "", "Y 坐标。", "Y coordinate."),
            field("z", "f64", "", "Z 坐标。", "Z coordinate."),
            field("yaw", "f32", "", "水平朝向。", "Yaw."),
            field("pitch", "f32", "", "俯仰角。", "Pitch."),
        ],
    },
    PluginStructDoc {
        name: "NpcMutationQuery",
        zh: "NPC 变更查询。",
        en: "NPC mutation query.",
        fields: &[field(
            "reason",
            "String",
            "",
            "触发原因。",
            "Reason for the query.",
        )],
    },
    PluginStructDoc {
        name: "NpcMutationResponse",
        zh: "NPC 变更响应。",
        en: "NPC mutation response.",
        fields: &[field(
            "operations",
            "Vec<NpcMutationOp>",
            "[]",
            "变更操作列表。",
            "Mutation operations.",
        )],
    },
    PluginStructDoc {
        name: "NpcMutationOp",
        zh: "NPC 变更操作枚举：Upsert 或 Remove。",
        en: "NPC mutation operation enum: Upsert or Remove.",
        fields: &[
            field(
                "Upsert { npc }",
                "NpcUpsert",
                "",
                "新增或更新 NPC。",
                "Adds or updates an NPC.",
            ),
            field(
                "Remove { key }",
                "String",
                "",
                "移除指定 NPC。",
                "Removes an NPC by key.",
            ),
        ],
    },
    PluginStructDoc {
        name: "NpcUpsert",
        zh: "NPC 新增/更新数据。",
        en: "NPC upsert data.",
        fields: &[
            field("key", "String", "", "NPC 稳定键。", "Stable NPC key."),
            field(
                "dimension",
                "String",
                "\"minecraft:overworld\"",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field("x", "f64", "", "X 坐标。", "X coordinate."),
            field("y", "f64", "", "Y 坐标。", "Y coordinate."),
            field("z", "f64", "", "Z 坐标。", "Z coordinate."),
            field("yaw", "f32", "0", "水平朝向。", "Yaw."),
            field("pitch", "f32", "0", "俯仰角。", "Pitch."),
            field("name", "String", "\"\"", "NPC 玩家名。", "NPC player name."),
            field(
                "display_name",
                "String",
                "\"\"",
                "显示名称。",
                "Display name.",
            ),
            field(
                "entity_type",
                "String",
                "\"\"",
                "客户端看到的原版实体类型。留空时 NPC 仍以玩家实体显示；例如 people 可套壳为 minecraft:zombie。",
                "Vanilla entity type shown to clients. Empty keeps the NPC as a player entity; for example people can be disguised as minecraft:zombie.",
            ),
            field(
                "skin_textures",
                "String",
                "\"\"",
                "Mojang texture 属性。",
                "Mojang texture property.",
            ),
            field(
                "skin_signature",
                "String",
                "\"\"",
                "Mojang texture 签名。",
                "Mojang texture signature.",
            ),
            field(
                "look_at_players",
                "bool",
                "false",
                "是否看向附近玩家。",
                "Whether the NPC looks at nearby players.",
            ),
            field(
                "main_hand_event",
                "String",
                "\"interact\"",
                "主手交互事件名。",
                "Main-hand interaction event name.",
            ),
            field(
                "off_hand_event",
                "String",
                "\"interact_off_hand\"",
                "副手交互事件名。",
                "Off-hand interaction event name.",
            ),
            field(
                "attack_event",
                "String",
                "\"attack\"",
                "攻击交互事件名。",
                "Attack interaction event name.",
            ),
        ],
    },
    PluginStructDoc {
        name: "CustomEntityDefinition",
        zh: "插件自定义实体定义。",
        en: "Plugin-defined custom entity definition.",
        fields: &[
            field(
                "id",
                "String",
                "",
                "自定义实体 ID，例如 demo:patrol_guard。",
                "Custom entity ID, for example demo:patrol_guard.",
            ),
            field(
                "entity_type",
                "String",
                "\"\"",
                "旧版套壳实体类型别名；新插件请使用 shell_entity_type。",
                "Legacy shell entity type alias; new plugins should use shell_entity_type.",
            ),
            field(
                "shell_entity_type",
                "String",
                "\"\"",
                "客户端看到的实体类型。留空时使用 id，例如 demo:patrol_guard。",
                "Entity type shown to clients. Empty uses id, for example demo:patrol_guard.",
            ),
            field(
                "registry_id",
                "Option<i32>",
                "None",
                "无套壳模组实体的可选协议 registry id。",
                "Optional protocol registry id for an undisguised modded entity.",
            ),
            field(
                "display_name",
                "String",
                "\"\"",
                "默认显示名称。",
                "Default display name.",
            ),
            field(
                "ai",
                "String",
                "\"\"",
                "默认 AI 标识，例如 plugin:demo_patrol。",
                "Default AI identifier, for example plugin:demo_patrol.",
            ),
            field(
                "ai_params",
                "BTreeMap<String, serde_json::Value>",
                "{}",
                "Structured plugin AI parameters.",
                "Structured plugin AI parameters.",
            ),
        ],
    },
    PluginStructDoc {
        name: "CustomEntityRegistryResponse",
        zh: "自定义实体注册响应。",
        en: "Custom entity registry response.",
        fields: &[field(
            "entities",
            "Vec<CustomEntityDefinition>",
            "[]",
            "自定义实体定义列表。",
            "Custom entity definitions.",
        )],
    },
    PluginStructDoc {
        name: "EntityAiTickQuery",
        zh: "实体 AI tick 查询。",
        en: "Entity AI tick query.",
        fields: &[
            field(
                "entity",
                "EntityAiEntityPayload",
                "",
                "当前实体。",
                "Current entity.",
            ),
            field(
                "nearby_players",
                "Vec<EntityAiPlayerPayload>",
                "[]",
                "附近玩家。",
                "Nearby players.",
            ),
            field(
                "tick_ms",
                "u64",
                "",
                "AI tick 间隔毫秒数。",
                "AI tick interval in milliseconds.",
            ),
        ],
    },
    PluginStructDoc {
        name: "EntityAiEntityPayload",
        zh: "AI tick 中的实体信息。",
        en: "Entity information for an AI tick.",
        fields: &[
            field("key", "String", "", "实体稳定键。", "Stable entity key."),
            field(
                "entity_id",
                "i32",
                "",
                "服务端实体 ID。",
                "Server entity ID.",
            ),
            field(
                "entity_type",
                "String",
                "",
                "客户端看到的实体类型。",
                "Entity type shown to clients.",
            ),
            field(
                "custom_type",
                "String",
                "\"\"",
                "插件自定义实体类型。",
                "Plugin custom entity type.",
            ),
            field("ai", "String", "\"\"", "AI 标识。", "AI identifier."),
            field(
                "spawn_rule",
                "String",
                "\"\"",
                "产生该实体的刷怪规则。",
                "Spawn rule that produced the entity.",
            ),
            field(
                "ai_params",
                "BTreeMap<String, serde_json::Value>",
                "{}",
                "Structured plugin AI parameters.",
                "Structured plugin AI parameters.",
            ),
            field(
                "dimension",
                "String",
                "",
                "维度资源键。",
                "Dimension resource key.",
            ),
            field(
                "position",
                "PlayerPositionPayload",
                "",
                "实体位置。",
                "Entity position.",
            ),
        ],
    },
    PluginStructDoc {
        name: "EntityAiPlayerPayload",
        zh: "AI tick 附近玩家信息。",
        en: "Nearby player information for an AI tick.",
        fields: &[
            field("player", "PlayerPayloadOwned", "", "玩家。", "Player."),
            field(
                "position",
                "PlayerPositionPayload",
                "",
                "玩家位置。",
                "Player position.",
            ),
        ],
    },
    PluginStructDoc {
        name: "EntityAiTickResponse",
        zh: "实体 AI tick 响应。",
        en: "Entity AI tick response.",
        fields: &[field(
            "operations",
            "Vec<EntityAiOperation>",
            "[]",
            "AI 操作列表。",
            "AI operations.",
        )],
    },
    PluginStructDoc {
        name: "EntityAiOperation",
        zh: "实体 AI 操作枚举：MoveDelta、LookAt 或 Remove。",
        en: "Entity AI operation enum: MoveDelta, LookAt, or Remove.",
        fields: &[
            field(
                "MoveDelta { x, y, z, yaw, pitch }",
                "f64 / Option<f32>",
                "",
                "按增量移动实体，可选更新朝向。",
                "Moves the entity by a delta and optionally updates rotation.",
            ),
            field(
                "LookAt { x, y, z }",
                "f64",
                "",
                "让实体看向指定坐标。",
                "Makes the entity look at a position.",
            ),
            field("Remove", "unit", "", "移除实体。", "Removes the entity."),
        ],
    },
    PluginStructDoc {
        name: "PlayerAction",
        zh: "服务端对玩家执行的动作枚举。",
        en: "Server-side action enum for a player.",
        fields: &[
            field(
                "SystemMessage",
                "{ text, translate, with, overlay }",
                "",
                "发送系统消息。",
                "Sends a system message.",
            ),
            field(
                "Teleport",
                "{ dimension, x, y, z, yaw, pitch }",
                "",
                "传送玩家。",
                "Teleports the player.",
            ),
            field(
                "Transfer",
                "{ host, port, message }",
                "",
                "使用客户端转移连接。",
                "Transfers the client connection.",
            ),
            field(
                "ProxyConnect",
                "{ server, message }",
                "",
                "通过代理连接到子服。",
                "Connects through the proxy to a backend server.",
            ),
            field(
                "OpenMenu",
                "{ menu }",
                "",
                "打开配置菜单。",
                "Opens a configured menu.",
            ),
            field(
                "SetPlayersVisible",
                "{ visible }",
                "",
                "切换其他玩家可见性。",
                "Toggles visibility of other players.",
            ),
            field(
                "Velocity",
                "{ x, y, z, additive }",
                "",
                "设置或叠加玩家速度。",
                "Sets or adds player velocity.",
            ),
        ],
    },
];

const PLUGIN_HOST_API_DOCS: &[PluginHostApiDoc] = &[
    PluginHostApiDoc {
        name: "log",
        signature: "qexed::log(ptr: i32, len: i32)",
        wrapper: "qexed_plugin_sdk::log(message)",
        zh: "写入服务端插件日志。",
        en: "Writes a server-side plugin log line.",
    },
    PluginHostApiDoc {
        name: "config_exists",
        signature: "qexed::config_exists(ptr: i32, len: i32) -> i32",
        wrapper: "qexed_plugin_sdk::config_exists(path)",
        zh: "检查插件配置文件是否存在。",
        en: "Checks whether a plugin config file exists.",
    },
    PluginHostApiDoc {
        name: "config_read",
        signature: "qexed::config_read(path_ptr: i32, path_len: i32, out_ptr: i32, out_len: i32) -> i64",
        wrapper: "qexed_plugin_sdk::config_read(path)",
        zh: "读取插件配置文件。返回负数时表示输出缓冲区需要扩大。",
        en: "Reads a plugin config file. A negative return value asks the caller to grow the output buffer.",
    },
    PluginHostApiDoc {
        name: "config_write",
        signature: "qexed::config_write(path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32) -> i32",
        wrapper: "qexed_plugin_sdk::config_write(path, contents)",
        zh: "写入插件配置文件。",
        en: "Writes a plugin config file.",
    },
    PluginHostApiDoc {
        name: "economy_register_currency",
        signature: "qexed::economy_register_currency(id_ptr, id_len, name_ptr, name_len, symbol_ptr, symbol_len, fractional_digits) -> i32",
        wrapper: "qexed_plugin_sdk::economy_register_currency(id, name, symbol, fractional_digits)",
        zh: "注册经济货币。",
        en: "Registers an economy currency.",
    },
    PluginHostApiDoc {
        name: "economy_currency_info",
        signature: "qexed::economy_currency_info(currency_ptr, currency_len, out_ptr, out_len) -> i64",
        wrapper: "qexed_plugin_sdk::economy_currency_info(currency)",
        zh: "读取经济货币信息。",
        en: "Reads economy currency information.",
    },
    PluginHostApiDoc {
        name: "economy_balance",
        signature: "qexed::economy_balance(player_ptr, player_len, currency_ptr, currency_len) -> i64",
        wrapper: "qexed_plugin_sdk::economy_balance(player, currency)",
        zh: "读取玩家余额。",
        en: "Reads a player's balance.",
    },
    PluginHostApiDoc {
        name: "economy_set_balance",
        signature: "qexed::economy_set_balance(player_ptr, player_len, currency_ptr, currency_len, amount) -> i64",
        wrapper: "qexed_plugin_sdk::economy_set_balance(player, currency, amount)",
        zh: "设置玩家余额。",
        en: "Sets a player's balance.",
    },
    PluginHostApiDoc {
        name: "economy_deposit",
        signature: "qexed::economy_deposit(player_ptr, player_len, currency_ptr, currency_len, amount) -> i64",
        wrapper: "qexed_plugin_sdk::economy_deposit(player, currency, amount)",
        zh: "给玩家增加余额。",
        en: "Deposits into a player's balance.",
    },
    PluginHostApiDoc {
        name: "economy_withdraw",
        signature: "qexed::economy_withdraw(player_ptr, player_len, currency_ptr, currency_len, amount) -> i64",
        wrapper: "qexed_plugin_sdk::economy_withdraw(player, currency, amount)",
        zh: "扣除玩家余额。",
        en: "Withdraws from a player's balance.",
    },
    PluginHostApiDoc {
        name: "lottery_roll",
        signature: "qexed::lottery_roll(entries_ptr: i32, entries_len: i32, out_ptr: i32, out_len: i32) -> i64",
        wrapper: "qexed_plugin_sdk::lottery_roll(entries)",
        zh: "按权重随机抽取一个值。",
        en: "Rolls one weighted random value.",
    },
    PluginHostApiDoc {
        name: "pathfinding_find",
        signature: "qexed::pathfinding_find(query_ptr: i32, query_len: i32, out_ptr: i32, out_len: i32) -> i64",
        wrapper: "qexed_plugin_sdk::pathfinding_find(dimension, start, goal, max_nodes)",
        zh: "请求服务端寻路服务返回路径节点。",
        en: "Asks the server pathfinding service for path nodes.",
    },
];

const fn field(
    name: &'static str,
    ty: &'static str,
    default: &'static str,
    zh: &'static str,
    en: &'static str,
) -> PluginFieldDoc {
    PluginFieldDoc {
        name,
        ty,
        default,
        zh,
        en,
    }
}

#[derive(Debug, Default)]
struct NoticeMaps {
    warnings: BTreeMap<String, String>,
    dangers: BTreeMap<String, String>,
    pending_deprecated: BTreeMap<String, String>,
    deprecated: BTreeMap<String, String>,
    migration_notices: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
struct UiText {
    site_title: &'static str,
    commit: &'static str,
    language: &'static str,
    config_file: &'static str,
    field_count: &'static str,
    path: &'static str,
    value_type: &'static str,
    default_value: &'static str,
    current_value: &'static str,
    description: &'static str,
    notice: &'static str,
    none: &'static str,
    not_loaded: &'static str,
    missing: &'static str,
    editor_title: &'static str,
    editor_subtitle: &'static str,
    upload: &'static str,
    clear: &'static str,
    copy: &'static str,
    copied: &'static str,
    paste_placeholder: &'static str,
    parse_ok: &'static str,
    parse_error: &'static str,
    matched_fields: &'static str,
    file_label: &'static str,
    danger: &'static str,
    warning: &'static str,
    pending_deprecated: &'static str,
    deprecated: &'static str,
    migration: &'static str,
    target: &'static str,
    asset: &'static str,
    index: &'static str,
    home: &'static str,
    app_count: &'static str,
    field_unit: &'static str,
    summary: &'static str,
    complex_details: &'static str,
    site_description_prefix: &'static str,
}

fn ui_text(lang: &str) -> UiText {
    if lang.to_ascii_lowercase().starts_with("zh") {
        UiText {
            site_title: "Qexed 配置文档",
            commit: "提交",
            language: "语言",
            config_file: "配置文件",
            field_count: "字段数量",
            path: "配置项",
            value_type: "类型",
            default_value: "默认值",
            current_value: "当前值",
            description: "说明",
            notice: "提示",
            none: "无",
            not_loaded: "粘贴或上传 TOML 后预览当前值。",
            missing: "未提供",
            editor_title: "TOML 在线预览",
            editor_subtitle: "粘贴或上传配置文件，文档中的匹配字段会显示当前值。",
            upload: "上传 TOML",
            clear: "清空",
            copy: "复制",
            copied: "已复制",
            paste_placeholder: "在这里粘贴 TOML...",
            parse_ok: "TOML 已解析",
            parse_error: "TOML 解析失败",
            matched_fields: "个字段匹配",
            file_label: "TOML",
            danger: "危险",
            warning: "警告",
            pending_deprecated: "即将废弃",
            deprecated: "已废弃",
            migration: "迁移",
            target: "目标",
            asset: "资源",
            index: "索引",
            home: "首页",
            app_count: "应用数量",
            field_unit: "个字段",
            summary: "目录",
            complex_details: "复杂类型详情",
            site_description_prefix: "AutoDoc 生成自提交",
        }
    } else {
        UiText {
            site_title: "Qexed Config Docs",
            commit: "Commit",
            language: "Language",
            config_file: "Config file",
            field_count: "Field count",
            path: "Path",
            value_type: "Type",
            default_value: "Default",
            current_value: "Current",
            description: "Description",
            notice: "Notice",
            none: "None",
            not_loaded: "Paste or upload TOML to preview current values.",
            missing: "Not provided",
            editor_title: "TOML preview",
            editor_subtitle: "Paste or upload a config file. Matching fields update in the table.",
            upload: "Upload TOML",
            clear: "Clear",
            copy: "Copy",
            copied: "Copied",
            paste_placeholder: "Paste TOML here...",
            parse_ok: "TOML parsed",
            parse_error: "TOML parse error",
            matched_fields: "matched fields",
            file_label: "TOML",
            danger: "Danger",
            warning: "Warning",
            pending_deprecated: "Pending deprecated",
            deprecated: "Deprecated",
            migration: "Migration",
            target: "Target",
            asset: "Asset",
            index: "Index",
            home: "Home",
            app_count: "Apps",
            field_unit: "fields",
            summary: "Summary",
            complex_details: "Complex Type Details",
            site_description_prefix: "AutoDoc generated at",
        }
    }
}

fn bundle_ui_text(bundle: &DocBundle) -> UiText {
    bundle
        .languages
        .first()
        .map(|language_docs| ui_text(&language_docs.lang))
        .unwrap_or_else(|| ui_text("en"))
}

fn main() -> Result<()> {
    let args = Args::parse();
    let langs = normalized_langs(args.langs);
    let formats = normalized_formats(args.formats);
    let commit = args
        .commit
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| crate::COMMIT_HASH.to_string());

    let output_root = args.out.join(&commit);
    if !args.placeholders_only {
        fs::create_dir_all(&output_root)
            .with_context(|| format!("无法创建输出目录 {}", output_root.display()))?;

        let bundle = collect_bundle(&commit, &langs)?;
        write_assets(&output_root)?;

        if formats.contains(&OutputFormat::Markdown) {
            write_markdown_docs(&output_root, &bundle, false)?;
        }
        if formats.contains(&OutputFormat::Mdx) {
            write_markdown_docs(&output_root, &bundle, true)?;
        }
        if formats.contains(&OutputFormat::NextApp) {
            write_next_app_docs(&output_root, &bundle)?;
        }
        if formats.contains(&OutputFormat::NextPages) {
            write_next_pages_docs(&output_root, &bundle)?;
        }
        if formats.contains(&OutputFormat::Mkdocs) {
            write_mkdocs_docs(&output_root, &bundle)?;
        }
        if formats.contains(&OutputFormat::Mdbook) {
            write_mdbook_docs(&output_root, &bundle)?;
        }
        if formats.contains(&OutputFormat::Json) {
            write_json_docs(&output_root, &bundle)?;
        }

        write_manifest(&output_root, &bundle, &formats)?;
    }
    if let Some(plugin_docs_out) = args.plugin_docs_out {
        write_plugin_api_next_app_docs(&plugin_docs_out, &commit, &langs)?;
    }
    if let Some(placeholder_docs_out) = args.placeholder_docs_out {
        write_placeholder_next_app_docs(&placeholder_docs_out, &commit, &langs)?;
    }

    if args.placeholders_only {
        println!("generated placeholder docs");
    } else {
        println!("generated config docs: {}", output_root.display());
    }
    Ok(())
}

fn normalized_langs(langs: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let source = if langs.is_empty() {
        DEFAULT_LANGS
            .iter()
            .map(|lang| lang.to_string())
            .collect::<Vec<_>>()
    } else {
        langs
    };

    source
        .into_iter()
        .filter_map(|lang| {
            let lang = lang.trim().to_string();
            if lang.is_empty() || !seen.insert(lang.clone()) {
                None
            } else {
                Some(lang)
            }
        })
        .collect()
}

fn normalized_formats(formats: Vec<OutputFormat>) -> Vec<OutputFormat> {
    if formats.is_empty() || formats.contains(&OutputFormat::All) {
        return vec![
            OutputFormat::Markdown,
            OutputFormat::Mdx,
            OutputFormat::NextApp,
            OutputFormat::NextPages,
            OutputFormat::Mkdocs,
            OutputFormat::Mdbook,
            OutputFormat::Json,
        ];
    }

    let mut result = Vec::new();
    for format in formats {
        if format != OutputFormat::All && !result.contains(&format) {
            result.push(format);
        }
    }
    result
}

fn collect_bundle(commit: &str, langs: &[String]) -> Result<DocBundle> {
    let mut languages = Vec::new();

    for lang in langs {
        languages.push(LanguageDocs {
            lang: lang.clone(),
            apps: collect_language_apps(lang)?,
        });
    }

    Ok(DocBundle {
        commit: commit.to_string(),
        languages,
    })
}

fn collect_language_apps(lang: &str) -> Result<Vec<DocApp>> {
    let mut apps = Vec::new();
    apps.push(collect_app::<Qexed>("qexed", "qexed.toml", lang)?);
    apps.push(collect_app::<QexedPluginDownload>(
        "qexed_plugin_download",
        "qexed_plugin_download.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedServer>(
        "qexed_server",
        "qexed_server.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedProxy>("proxy", "proxy.toml", lang)?);
    apps.push(collect_app::<QexedLanDiscovery>(
        "qexed_lan_discovery",
        "qexed_lan_discovery.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedContentFilter>(
        "qexed_content_filter",
        "qexed_content_filter.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedEntity>(
        "qexed_entity",
        "qexed_entity.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedNpc>(
        "qexed_npc",
        "qexed_npc.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedEntityRendering>(
        "qexed_entity_rendering",
        "qexed_entity_rendering.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedLobby>(
        "qexed_lobby",
        "qexed_lobby.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedMenus>(
        "qexed_menus",
        "qexed_menus.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedPermissions>(
        "qexed_permissions",
        "qexed_permissions.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedPlaceholders>(
        "qexed_placeholders",
        "qexed_placeholders.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedPlayerAudit>(
        "qexed_player_audit",
        "qexed_player_audit.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedPlayerData>(
        "qexed_player_data",
        "qexed_player_data.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedPlayerMessages>(
        "qexed_player_messages",
        "qexed_player_messages.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedResourcePack>(
        "qexed_resource_pack",
        "qexed_resource_pack.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedScoreboard>(
        "qexed_scoreboard",
        "qexed_scoreboard.toml",
        lang,
    )?);
    apps.push(collect_app::<World>("world", "world.toml", lang)?);
    apps.push(collect_app::<QexedWarden>(
        "qexed_warden",
        "qexed_warden.toml",
        lang,
    )?);
    apps.push(collect_app::<QexedIpConnectionSpeedTest>(
        "qexed_ip_connect_speed_test",
        "qexed_ip_connect_speed_test.toml",
        lang,
    )?);
    Ok(apps)
}

fn collect_app<T>(name: &str, config_file: &str, lang: &str) -> Result<DocApp>
where
    T: AppConfigTrait + AutoDocConfigTrait + Serialize + Default,
{
    collect_app_with_scope::<T>(name, config_file, lang, None)
}

fn collect_app_with_scope<T>(
    name: &str,
    config_file: &str,
    lang: &str,
    root_path: Option<&str>,
) -> Result<DocApp>
where
    T: AppConfigTrait + AutoDocConfigTrait + Serialize + Default,
{
    let defaults = serde_json::to_value(T::default())
        .with_context(|| format!("无法序列化 {name} 默认配置"))?;
    let notices = NoticeMaps {
        warnings: into_map(T::warning_fields(lang)),
        dangers: into_map(T::danger_fields(lang)),
        pending_deprecated: into_map(T::pending_deprecated_fields(lang)),
        deprecated: into_map(T::deprecation_fields(lang)),
        migration_notices: into_map(T::migration_notice_fields(lang)),
    };
    let default_display = into_map(T::default_display_fields(lang));
    let value_types = into_type_map(T::field_value_types());

    let split_root_paths = split_root_paths::<T>();
    let mut fields = Vec::new();
    for (path, description) in scoped_entries(T::doc_fields(lang), root_path) {
        let full_path = full_field_path(root_path, &path);
        if split_scope_excludes(root_path, &full_path, &split_root_paths) {
            continue;
        }
        let default = value_at_path(&defaults, &full_path);
        fields.push(DocField {
            value_type: resolved_value_type(default, value_types.get(&full_path).copied())
                .to_string(),
            default_value: default_display
                .get(&full_path)
                .cloned()
                .or_else(|| default.and_then(display_default_value)),
            warning: notices.warnings.get(&full_path).cloned(),
            danger: notices.dangers.get(&full_path).cloned(),
            pending_deprecated: notices.pending_deprecated.get(&full_path).cloned(),
            deprecated: notices.deprecated.get(&full_path).cloned(),
            migration_notice: notices.migration_notices.get(&full_path).cloned(),
            path,
            description,
        });
    }

    let parent_paths = fields
        .iter()
        .filter_map(|field| {
            field
                .path
                .rsplit_once('.')
                .map(|(parent, _)| parent.to_string())
        })
        .collect::<BTreeSet<_>>();
    for field in &mut fields {
        if field.value_type == "unknown" && parent_paths.contains(&field.path) {
            field.value_type = "array".to_string();
        }
    }

    for field in &mut fields {
        if field.value_type == "array" && parent_paths.contains(&field.path) {
            field.default_value = None;
        }
    }

    fields.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(DocApp {
        name: name.to_string(),
        config_file: config_file.to_string(),
        config_path: config_path::<T>(config_file, root_path),
        description: T::config_file_description(lang, config_file, root_path),
        root_path: root_path.map(str::to_string),
        fields,
    })
}

fn split_root_paths<T: AppConfigTrait>() -> Vec<&'static str> {
    T::split_config_files()
        .iter()
        .map(|split| split.root_path)
        .collect()
}

fn scoped_path_contains(root_path: &str, path: &str) -> bool {
    path == root_path
        || path
            .strip_prefix(root_path)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn split_scope_excludes(
    current_root_path: Option<&str>,
    full_path: &str,
    split_root_paths: &[&'static str],
) -> bool {
    split_root_paths.iter().any(|split_root_path| {
        if Some(*split_root_path) == current_root_path {
            return false;
        }
        match current_root_path {
            Some(current_root_path) => {
                split_root_path
                    .strip_prefix(current_root_path)
                    .is_some_and(|suffix| suffix.starts_with('.'))
                    && scoped_path_contains(split_root_path, full_path)
            }
            None => scoped_path_contains(split_root_path, full_path),
        }
    })
}

fn config_doc_files(language_docs: &LanguageDocs) -> Vec<ConfigDocFile> {
    language_docs
        .apps
        .iter()
        .map(|app| ConfigDocFile {
            name: app.name.clone(),
            config_path: app.config_path.clone(),
            route: app.name.clone(),
            description: app.description.clone(),
        })
        .collect()
}

fn into_map(values: Vec<(String, String)>) -> BTreeMap<String, String> {
    values.into_iter().collect()
}

fn into_type_map(values: Vec<(String, &'static str)>) -> BTreeMap<String, &'static str> {
    values.into_iter().collect()
}

fn config_path<T: AppConfigTrait>(config_file: &str, root_path: Option<&str>) -> String {
    let base = T::PATH.trim_matches('/');
    let config_file = scoped_config_file::<T>(config_file, root_path);
    if base.is_empty() {
        format!("config/{config_file}")
    } else {
        format!("config/{base}/{config_file}")
    }
}

fn scoped_config_file<T: AppConfigTrait>(config_file: &str, root_path: Option<&str>) -> String {
    if root_path.is_some() {
        format!("{}.d/{config_file}", T::NAME)
    } else {
        config_file.to_string()
    }
}

fn scoped_entries(
    entries: Vec<(String, String)>,
    root_path: Option<&str>,
) -> Vec<(String, String)> {
    let Some(root_path) = root_path else {
        return entries;
    };
    let local_root_path = split_file_local_root_path(root_path);
    entries
        .into_iter()
        .filter_map(|(path, description)| {
            if path == root_path {
                Some((local_root_path.to_string(), description))
            } else {
                path.strip_prefix(&format!("{root_path}."))
                    .map(|suffix| (format!("{local_root_path}.{suffix}"), description))
            }
        })
        .collect()
}

fn full_field_path(root_path: Option<&str>, path: &str) -> String {
    match root_path {
        Some(root_path) => {
            let local_root_path = split_file_local_root_path(root_path);
            if path == local_root_path {
                root_path.to_string()
            } else if let Some(suffix) = path.strip_prefix(&format!("{local_root_path}.")) {
                format!("{root_path}.{suffix}")
            } else {
                format!("{root_path}.{path}")
            }
        }
        None => path.to_string(),
    }
}

fn split_file_local_root_path(root_path: &str) -> &str {
    root_path.strip_prefix("server.").unwrap_or(root_path)
}

fn value_at_path<'a>(root: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    let mut current = root;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn resolved_value_type(
    default: Option<&JsonValue>,
    type_hint: Option<&'static str>,
) -> &'static str {
    match default {
        Some(JsonValue::Null) => type_hint.unwrap_or("null"),
        Some(value) => value_type_name(value),
        None => type_hint.unwrap_or("unknown"),
    }
}

fn value_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(number) if number.is_i64() || number.is_u64() => "integer",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn display_default_value(value: &JsonValue) -> Option<String> {
    if value.is_null() {
        return None;
    }

    Some(toml_default_value(value))
}

fn toml_default_value(value: &JsonValue) -> String {
    match value {
        JsonValue::Object(values) => render_toml_object(values, ""),
        _ => render_toml_value(value),
    }
}

fn render_toml_object(values: &serde_json::Map<String, JsonValue>, prefix: &str) -> String {
    let mut assignments = Vec::new();
    let mut sections = Vec::new();

    for (key, value) in values {
        if value.is_null() {
            continue;
        }

        if let JsonValue::Object(nested) = value {
            let section = if prefix.is_empty() {
                key.to_string()
            } else {
                format!("{prefix}.{key}")
            };
            let body = render_toml_object(nested, &section);
            if !body.is_empty() {
                sections.push(format!("[{section}]\n{body}"));
            }
        } else {
            assignments.push(format!(
                "{} = {}",
                quote_toml_key(key),
                render_toml_value(value)
            ));
        }
    }

    [assignments.join("\n"), sections.join("\n\n")]
        .into_iter()
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_toml_value(value: &JsonValue) -> String {
    match sanitize_default_value(value) {
        JsonValue::Null => "\"<null>\"".to_string(),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Number(value) => value.to_string(),
        JsonValue::String(value) => quote_toml_string(&value),
        JsonValue::Array(values) => {
            let values = values
                .iter()
                .map(render_toml_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        JsonValue::Object(values) => {
            let entries = values
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| {
                    format!("{} = {}", quote_toml_key(key), render_toml_value(value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {entries} }}")
        }
    }
}

fn quote_toml_key(key: &str) -> String {
    if key
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '-')
    {
        key.to_string()
    } else {
        quote_toml_string(key)
    }
}

fn quote_toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("JSON string serialization should not fail")
}

fn sanitize_default_value(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(values) => {
            JsonValue::Array(values.iter().map(sanitize_default_value).collect())
        }
        JsonValue::Object(values) => JsonValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), sanitize_default_value(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn write_assets(output_root: &Path) -> Result<()> {
    let asset_dir = output_root.join("assets");
    fs::create_dir_all(&asset_dir)
        .with_context(|| format!("无法创建资源目录 {}", asset_dir.display()))?;

    let logo_target = asset_dir.join("logo.ico");
    fs::write(&logo_target, LOGO_BYTES)
        .with_context(|| format!("无法写入内嵌图标 {}", logo_target.display()))?;
    Ok(())
}

fn write_markdown_docs(output_root: &Path, bundle: &DocBundle, mdx: bool) -> Result<()> {
    let dir = output_root.join(if mdx { "mdx" } else { "markdown" });
    fs::create_dir_all(&dir).with_context(|| format!("无法创建目录 {}", dir.display()))?;

    for language_docs in &bundle.languages {
        let lang_dir = dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建目录 {}", lang_dir.display()))?;

        for app in &language_docs.apps {
            let extension = if mdx { "mdx" } else { "md" };
            let path = lang_dir.join(format!("{}.{}", app.name, extension));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, mdx);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    Ok(())
}

fn write_next_pages_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let pages_dir = output_root.join("next-pages").join("pages").join("config");
    fs::create_dir_all(&pages_dir)
        .with_context(|| format!("无法创建 Next.js pages 目录 {}", pages_dir.display()))?;

    let index_path = pages_dir.join("index.mdx");
    fs::write(&index_path, render_next_index(bundle))
        .with_context(|| format!("无法写入 {}", index_path.display()))?;

    for language_docs in &bundle.languages {
        let lang_dir = pages_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建目录 {}", lang_dir.display()))?;

        let meta = json!({
            "title": format!("Qexed Config {}", language_docs.lang),
            "pages": language_docs.apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        });
        let meta_path = lang_dir.join("_meta.json");
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("无法写入 {}", meta_path.display()))?;

        for app in &language_docs.apps {
            let path = lang_dir.join(format!("{}.mdx", app.name));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, true);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    Ok(())
}

fn write_next_app_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let docs_dir = output_root
        .join("next-app")
        .join("app")
        .join("docs")
        .join("config");
    fs::create_dir_all(&docs_dir)
        .with_context(|| format!("无法创建 Next.js app 目录 {}", docs_dir.display()))?;

    let component_path = docs_dir.join("ConfigDocClient.tsx");
    fs::write(&component_path, CONFIG_DOC_CLIENT_TSX)
        .with_context(|| format!("无法写入 {}", component_path.display()))?;

    let index_path = docs_dir.join("page.mdx");
    fs::write(&index_path, render_next_app_index(bundle))
        .with_context(|| format!("无法写入 {}", index_path.display()))?;

    for language_docs in &bundle.languages {
        let lang_dir = docs_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建目录 {}", lang_dir.display()))?;

        let lang_index_path = lang_dir.join("page.mdx");
        fs::write(
            &lang_index_path,
            render_next_app_language_index(&bundle.commit, language_docs),
        )
        .with_context(|| format!("无法写入 {}", lang_index_path.display()))?;

        for app in &language_docs.apps {
            let app_dir = lang_dir.join(&app.name);
            fs::create_dir_all(&app_dir)
                .with_context(|| format!("无法创建目录 {}", app_dir.display()))?;

            let path = app_dir.join("page.mdx");
            let content = render_next_app_config_page(&bundle.commit, language_docs, app)?;
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    Ok(())
}

fn write_plugin_api_next_app_docs(output_dir: &Path, commit: &str, langs: &[String]) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("无法创建插件 API 文档目录 {}", output_dir.display()))?;

    let version_id = short_commit(commit);
    let version_dir = output_dir.join("dev").join(&version_id);
    fs::create_dir_all(&version_dir).with_context(|| {
        format!(
            "cannot create Plugin API version dir {}",
            version_dir.display()
        )
    })?;

    let langs = normalized_langs(langs.to_vec());
    for lang in langs {
        let lang_dir = version_dir.join(&lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("cannot create Plugin API lang dir {}", lang_dir.display()))?;
        let path = lang_dir.join("page.mdx");
        fs::write(&path, render_plugin_api_mdx(commit, &lang))
            .with_context(|| format!("无法写入 {}", path.display()))?;
    }

    fs::write(
        version_dir.join("page.mdx"),
        render_plugin_api_version_index(commit, &version_id),
    )
    .with_context(|| {
        format!(
            "cannot write Plugin API version index {}",
            version_dir.display()
        )
    })?;
    fs::write(
        output_dir.join("page.mdx"),
        render_plugin_api_root_index(&version_id),
    )
    .with_context(|| {
        format!(
            "cannot write Plugin API root index {}",
            output_dir.display()
        )
    })?;

    let stable_dir = output_dir.join("stable");
    fs::create_dir_all(&stable_dir).with_context(|| {
        format!(
            "cannot create Plugin API stable dir {}",
            stable_dir.display()
        )
    })?;
    fs::write(
        stable_dir.join("page.mdx"),
        render_plugin_api_empty_channel(true),
    )
    .with_context(|| "cannot write Plugin API stable page")?;

    let beta_dir = output_dir.join("beta");
    fs::create_dir_all(&beta_dir)
        .with_context(|| format!("cannot create Plugin API beta dir {}", beta_dir.display()))?;
    fs::write(
        beta_dir.join("page.mdx"),
        render_plugin_api_empty_channel(false),
    )
    .with_context(|| "cannot write Plugin API beta page")?;

    update_plugin_api_versions(output_dir, commit, &version_id)?;
    Ok(())
}

fn write_placeholder_next_app_docs(
    output_dir: &Path,
    commit: &str,
    langs: &[String],
) -> Result<()> {
    fs::create_dir_all(output_dir)
        .with_context(|| format!("无法创建 Placeholder 文档目录 {}", output_dir.display()))?;

    let version_id = short_commit(commit);
    let version_dir = output_dir.join("dev").join(&version_id);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("无法创建 Placeholder 版本目录 {}", version_dir.display()))?;

    let langs = normalized_langs(langs.to_vec());
    for lang in langs {
        let lang_dir = version_dir.join(&lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建 Placeholder 语言目录 {}", lang_dir.display()))?;
        fs::write(
            lang_dir.join("page.mdx"),
            render_placeholder_mdx(commit, &lang),
        )
        .with_context(|| format!("无法写入 Placeholder {}", lang_dir.display()))?;
    }

    fs::write(
        version_dir.join("page.mdx"),
        render_placeholder_version_index(commit, &version_id),
    )
    .with_context(|| format!("无法写入 Placeholder 版本首页 {}", version_dir.display()))?;
    fs::write(
        output_dir.join("page.mdx"),
        render_placeholder_root_index(&version_id),
    )
    .with_context(|| format!("无法写入 Placeholder 首页 {}", output_dir.display()))?;
    let stable_dir = output_dir.join("stable");
    fs::create_dir_all(&stable_dir)
        .with_context(|| format!("无法创建 Placeholder stable 目录 {}", stable_dir.display()))?;
    fs::write(
        stable_dir.join("page.mdx"),
        render_placeholder_empty_channel(true),
    )
    .with_context(|| "无法写入 Placeholder stable 页面")?;

    let beta_dir = output_dir.join("beta");
    fs::create_dir_all(&beta_dir)
        .with_context(|| format!("无法创建 Placeholder beta 目录 {}", beta_dir.display()))?;
    fs::write(
        beta_dir.join("page.mdx"),
        render_placeholder_empty_channel(false),
    )
    .with_context(|| "无法写入 Placeholder beta 页面")?;

    update_placeholder_versions(output_dir, commit, &version_id)?;
    Ok(())
}

fn update_placeholder_versions(output_dir: &Path, commit: &str, version_id: &str) -> Result<()> {
    let path = output_dir.join("versions.json");
    let mut versions = read_placeholder_versions(&path)?;
    let dev_versions = versions
        .as_object_mut()
        .and_then(|value| value.get_mut("dev"))
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| anyhow!("Placeholder versions.json 缺少 dev 数组"))?;

    let source_sha = commit.to_string();
    let exists = dev_versions.iter().any(|value| {
        value
            .get("id")
            .and_then(JsonValue::as_str)
            .is_some_and(|id| id == version_id)
    });

    if !exists {
        let entry = PlaceholderDocsVersion {
            channel: "dev".to_string(),
            id: version_id.to_string(),
            label: version_id.to_string(),
            source_ref: "v4".to_string(),
            source_sha,
            title: "Placeholder AutoDoc".to_string(),
            source_date: git_source_date(),
        };
        dev_versions.insert(0, serde_json::to_value(entry)?);
    }

    fs::write(&path, serde_json::to_string_pretty(&versions)?)
        .with_context(|| format!("无法写入 {}", path.display()))?;
    Ok(())
}

fn update_plugin_api_versions(output_dir: &Path, commit: &str, version_id: &str) -> Result<()> {
    let path = output_dir.join("versions.json");
    let mut versions = read_placeholder_versions(&path)?;
    let dev_versions = versions
        .as_object_mut()
        .and_then(|value| value.get_mut("dev"))
        .and_then(|value| value.as_array_mut())
        .ok_or_else(|| anyhow!("Plugin API versions.json missing dev array"))?;

    let source_sha = commit.to_string();
    let exists = dev_versions.iter().any(|value| {
        value
            .get("id")
            .and_then(JsonValue::as_str)
            .is_some_and(|id| id == version_id)
    });

    if !exists {
        let entry = PlaceholderDocsVersion {
            channel: "dev".to_string(),
            id: version_id.to_string(),
            label: version_id.to_string(),
            source_ref: "v4".to_string(),
            source_sha,
            title: "Plugin API AutoDoc".to_string(),
            source_date: git_source_date(),
        };
        dev_versions.insert(0, serde_json::to_value(entry)?);
    }

    fs::write(&path, serde_json::to_string_pretty(&versions)?)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

fn read_placeholder_versions(path: &Path) -> Result<JsonValue> {
    if path.exists() {
        let text =
            fs::read_to_string(path).with_context(|| format!("无法读取 {}", path.display()))?;
        return serde_json::from_str(&text).with_context(|| format!("无法解析 {}", path.display()));
    }

    Ok(json!({
        "stable": [],
        "beta": [],
        "dev": []
    }))
}

fn write_mkdocs_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let root = output_root.join("mkdocs");
    let docs_dir = root.join("docs");
    fs::create_dir_all(&docs_dir)
        .with_context(|| format!("无法创建 MkDocs docs 目录 {}", docs_dir.display()))?;

    let assets_dir = docs_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("无法创建 MkDocs assets 目录 {}", assets_dir.display()))?;
    fs::copy(
        output_root.join("assets").join("logo.ico"),
        assets_dir.join("logo.ico"),
    )
    .with_context(|| "无法复制 MkDocs 图标资源")?;

    fs::write(
        docs_dir.join("index.md"),
        render_site_index(bundle, "MkDocs"),
    )
    .with_context(|| "无法写入 MkDocs 首页")?;

    for language_docs in &bundle.languages {
        let lang_dir = docs_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建 MkDocs 语言目录 {}", lang_dir.display()))?;

        fs::write(
            lang_dir.join("index.md"),
            render_language_index(&bundle.commit, language_docs),
        )
        .with_context(|| format!("无法写入 MkDocs {} 首页", language_docs.lang))?;

        for app in &language_docs.apps {
            let path = lang_dir.join(format!("{}.md", app.name));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, false);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    fs::write(root.join("mkdocs.yml"), render_mkdocs_config(bundle))
        .with_context(|| "无法写入 mkdocs.yml")?;

    Ok(())
}

fn write_mdbook_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let root = output_root.join("mdbook");
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("无法创建 mdBook src 目录 {}", src_dir.display()))?;

    let assets_dir = src_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .with_context(|| format!("无法创建 mdBook assets 目录 {}", assets_dir.display()))?;
    fs::copy(
        output_root.join("assets").join("logo.ico"),
        assets_dir.join("logo.ico"),
    )
    .with_context(|| "无法复制 mdBook 图标资源")?;

    fs::write(
        src_dir.join("index.md"),
        render_site_index(bundle, "mdBook"),
    )
    .with_context(|| "无法写入 mdBook 首页")?;

    for language_docs in &bundle.languages {
        let lang_dir = src_dir.join(&language_docs.lang);
        fs::create_dir_all(&lang_dir)
            .with_context(|| format!("无法创建 mdBook 语言目录 {}", lang_dir.display()))?;

        fs::write(
            lang_dir.join("index.md"),
            render_language_index(&bundle.commit, language_docs),
        )
        .with_context(|| format!("无法写入 mdBook {} 首页", language_docs.lang))?;

        for app in &language_docs.apps {
            let path = lang_dir.join(format!("{}.md", app.name));
            let content = render_markdown_app(&bundle.commit, &language_docs.lang, app, false);
            fs::write(&path, content).with_context(|| format!("无法写入 {}", path.display()))?;
        }
    }

    fs::write(root.join("book.toml"), render_mdbook_config(bundle))
        .with_context(|| "无法写入 book.toml")?;
    fs::write(src_dir.join("SUMMARY.md"), render_mdbook_summary(bundle))
        .with_context(|| "无法写入 SUMMARY.md")?;

    Ok(())
}

fn write_json_docs(output_root: &Path, bundle: &DocBundle) -> Result<()> {
    let dir = output_root.join("json");
    fs::create_dir_all(&dir).with_context(|| format!("无法创建目录 {}", dir.display()))?;

    let path = dir.join("qexed_config_docs.json");
    fs::write(&path, serde_json::to_string_pretty(bundle)?)
        .with_context(|| format!("无法写入 {}", path.display()))?;

    Ok(())
}

fn write_manifest(output_root: &Path, bundle: &DocBundle, formats: &[OutputFormat]) -> Result<()> {
    let first_language = bundle
        .languages
        .first()
        .ok_or_else(|| anyhow!("缺少文档语言"))?;

    let manifest = Manifest {
        commit: bundle.commit.clone(),
        generator: "qexed_config_to_mdx",
        generated_formats: formats.iter().map(OutputFormat::name).collect(),
        languages: bundle
            .languages
            .iter()
            .map(|docs| docs.lang.clone())
            .collect(),
        apps: first_language
            .apps
            .iter()
            .map(|app| AppManifest {
                name: app.name.clone(),
                config_file: app.config_file.clone(),
                field_count: app.fields.len(),
            })
            .collect(),
        asset_logo: "assets/logo.ico".to_string(),
    };

    let path = output_root.join("manifest.json");
    fs::write(&path, serde_json::to_string_pretty(&manifest)?)
        .with_context(|| format!("无法写入 {}", path.display()))?;
    Ok(())
}

fn render_markdown_app(commit: &str, lang: &str, app: &DocApp, mdx: bool) -> String {
    let ui = ui_text(lang);
    let mut out = String::new();

    if mdx {
        out.push_str("---\n");
        out.push_str(&format!("title: \"{}\"\n", escape_yaml(&app.name)));
        out.push_str(&format!("commit: \"{}\"\n", escape_yaml(commit)));
        out.push_str(&format!("lang: \"{}\"\n", escape_yaml(lang)));
        out.push_str("---\n\n");
    }

    out.push_str(&format!("# {}\n\n", app.name));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!("- {}: `{lang}`\n", ui.language));
    out.push_str(&format!("- {}: `{}`\n", ui.config_file, app.config_path));
    out.push_str(&format!("- {}: `{}`\n", ui.field_count, app.fields.len()));
    out.push_str(&format!(
        "- {}: {}\n\n",
        ui.description,
        escape_markdown(&app.description)
    ));

    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n",
        ui.path, ui.value_type, ui.default_value, ui.description, ui.notice
    ));
    out.push_str("| --- | --- | --- | --- | --- |\n");

    for field in top_level_fields(app) {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} |\n",
            escape_markdown_table(&field.path),
            escape_markdown_table(&field.value_type),
            if is_complex_field(field) && has_direct_children(field, app) {
                render_details_link(&field.path, lang)
            } else {
                render_default_cell(field.default_value.as_deref(), ui.none)
            },
            escape_markdown_table(&field.description),
            render_notice_cell(field, &ui),
        ));
    }

    let complex_details = render_complex_details(app, &ui);
    if !complex_details.is_empty() {
        out.push('\n');
        out.push_str(&complex_details);
    }

    out
}

fn render_next_app_config_page(
    commit: &str,
    language_docs: &LanguageDocs,
    app: &DocApp,
) -> Result<String> {
    let lang = &language_docs.lang;
    let ui = ui_text(lang);
    let fields_json = serde_json::to_string_pretty(&app.fields)?;
    let config_docs_json = serde_json::to_string_pretty(&config_doc_files(language_docs))?;
    let labels_json = serde_json::to_string_pretty(&json!({
        "path": ui.path,
        "valueType": ui.value_type,
        "defaultValue": ui.default_value,
        "currentValue": ui.current_value,
        "description": ui.description,
        "notice": ui.notice,
        "none": ui.none,
        "notLoaded": ui.not_loaded,
        "missing": ui.missing,
        "editorTitle": ui.editor_title,
        "editorSubtitle": ui.editor_subtitle,
        "upload": ui.upload,
        "clear": ui.clear,
        "copy": ui.copy,
        "copied": ui.copied,
        "pastePlaceholder": ui.paste_placeholder,
        "parseOk": ui.parse_ok,
        "parseError": ui.parse_error,
        "matchedFields": ui.matched_fields,
        "fileLabel": ui.file_label,
        "complexDetails": ui.complex_details,
        "detailsLink": localized_text(lang, "\u{89c1} #{path}", "See #{path}"),
        "viewDetails": localized_text(lang, "\u{67e5}\u{770b}\u{8be6}\u{60c5}", "View details"),
        "fieldUnit": ui.field_unit,
        "danger": ui.danger,
        "warning": ui.warning,
        "pendingDeprecated": ui.pending_deprecated,
        "deprecated": ui.deprecated,
        "migration": ui.migration,
    }))?;

    let mut out = String::new();
    out.push_str("import {\n");
    out.push_str("  ConfigComplexDetails,\n");
    out.push_str("  ConfigDocEditor,\n");
    out.push_str("  ConfigDocProvider,\n");
    out.push_str("  ConfigFieldTable,\n");
    out.push_str("} from \"../../ConfigDocClient\";\n\n");
    out.push_str(&format!("export const configFields = {};\n\n", fields_json));
    out.push_str(&format!(
        "export const configDocs = {};\n\n",
        config_docs_json
    ));
    out.push_str(&format!("export const configLabels = {};\n\n", labels_json));

    out.push_str(&format!("# {}\n\n", app.name));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!("- {}: `{lang}`\n", ui.language));
    out.push_str(&format!("- {}: `{}`\n", ui.config_file, app.config_path));
    out.push_str(&format!("- {}: `{}`\n", ui.field_count, app.fields.len()));
    out.push_str(&format!(
        "- {}: {}\n\n",
        ui.description,
        escape_markdown(&app.description)
    ));
    out.push_str(&format!(
        "<ConfigDocProvider appName=\"{}\" configPath=\"{}\" configDocs={{configDocs}} commit=\"{}\" lang=\"{}\" fields={{configFields}} labels={{configLabels}}>\n\n",
        escape_mdx_attribute(&app.name),
        escape_mdx_attribute(&app.config_path),
        escape_mdx_attribute(commit),
        escape_mdx_attribute(lang)
    ));
    out.push_str("<ConfigDocEditor />\n\n");
    out.push_str("<ConfigFieldTable />\n\n");
    out.push_str("<ConfigComplexDetails />\n\n");
    out.push_str("</ConfigDocProvider>\n");

    Ok(out)
}

fn render_complex_details(app: &DocApp, ui: &UiText) -> String {
    let complex_fields = app
        .fields
        .iter()
        .filter(|field| is_complex_field(field) && has_direct_children(field, app))
        .collect::<Vec<_>>();
    if complex_fields.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!("## {}\n\n", ui.complex_details));

    for field in complex_fields {
        let child_prefix = format!("{}.", field.path);
        let children = app
            .fields
            .iter()
            .filter(|candidate| is_direct_child_path(&candidate.path, &child_prefix))
            .collect::<Vec<_>>();

        out.push_str(&format!(
            "### <span id=\"{}\"><code>{}</code></span>\n\n",
            details_id(&field.path),
            escape_mdx_text(&field.path)
        ));
        out.push_str(&format!(
            "- {}: `{}`\n",
            ui.value_type,
            escape_markdown_table(&field.value_type)
        ));
        if children.is_empty() {
            if let Some(default_value) = &field.default_value {
                out.push_str(&format!(
                    "- {}: `{}`\n",
                    ui.default_value,
                    escape_code_span(default_value)
                ));
            }
        } else {
            out.push_str(&format!(
                "- {}: {}\n",
                ui.default_value,
                escape_markdown(ui.none)
            ));
        }
        out.push_str(&format!(
            "- {}: {}\n",
            ui.description,
            escape_markdown(&field.description)
        ));
        let notice = render_notice_text(field, ui);
        if !notice.is_empty() {
            out.push_str(&format!("- {}: {}\n", ui.notice, notice));
        }

        if children.is_empty() {
            out.push('\n');
            continue;
        }

        out.push('\n');
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            ui.path, ui.value_type, ui.default_value, ui.description, ui.notice
        ));
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for child in children {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} | {} |\n",
                escape_markdown_table(&child.path),
                escape_markdown_table(&child.value_type),
                render_default_cell(child.default_value.as_deref(), ui.none),
                escape_markdown_table(&child.description),
                render_notice_cell(child, ui),
            ));
        }
        out.push('\n');
    }

    out
}

fn render_default_cell(value: Option<&str>, none_text: &str) -> String {
    value
        .map(|value| format!("`{}`", escape_code_span(value)))
        .unwrap_or_else(|| escape_markdown_table(none_text))
}

fn top_level_fields(app: &DocApp) -> Vec<&DocField> {
    app.fields
        .iter()
        .filter(|field| {
            !app.fields.iter().any(|candidate| {
                is_complex_field(candidate)
                    && field.path.starts_with(&format!("{}.", candidate.path))
            })
        })
        .collect()
}

fn is_complex_field(field: &DocField) -> bool {
    matches!(field.value_type.as_str(), "object" | "array")
}

fn has_direct_children(field: &DocField, app: &DocApp) -> bool {
    let child_prefix = format!("{}.", field.path);
    app.fields
        .iter()
        .any(|candidate| is_direct_child_path(&candidate.path, &child_prefix))
}

fn is_direct_child_path(path: &str, parent_prefix: &str) -> bool {
    path.strip_prefix(parent_prefix)
        .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('.'))
}

fn details_id(path: &str) -> String {
    let mut out = String::from("config-");
    let mut last_was_dash = false;

    for character in path.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            out.push(character);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }

    if out.ends_with('-') {
        out.pop();
    }

    out
}

fn render_details_link(path: &str, lang: &str) -> String {
    let text = localized_text(lang, "\u{89c1}", "See");
    format!(
        "[{} #{}](#{})",
        escape_markdown_table(text),
        escape_markdown_table(path),
        details_id(path)
    )
}

fn localized_text<'a>(lang: &str, zh: &'a str, en: &'a str) -> &'a str {
    if lang.to_ascii_lowercase().starts_with("zh") {
        zh
    } else {
        en
    }
}

fn render_notice_cell(field: &DocField, ui: &UiText) -> String {
    let mut notices = Vec::new();
    push_notice(&mut notices, ui.danger, field.danger.as_deref());
    push_notice(&mut notices, ui.warning, field.warning.as_deref());
    push_notice(
        &mut notices,
        ui.pending_deprecated,
        field.pending_deprecated.as_deref(),
    );
    push_notice(&mut notices, ui.deprecated, field.deprecated.as_deref());
    push_notice(
        &mut notices,
        ui.migration,
        field.migration_notice.as_deref(),
    );

    if notices.is_empty() {
        escape_markdown_table(ui.none)
    } else {
        notices
            .into_iter()
            .map(|notice| escape_markdown_table(&notice))
            .collect::<Vec<_>>()
            .join("<br />")
    }
}

fn render_notice_text(field: &DocField, ui: &UiText) -> String {
    let mut notices = Vec::new();
    push_notice(&mut notices, ui.danger, field.danger.as_deref());
    push_notice(&mut notices, ui.warning, field.warning.as_deref());
    push_notice(
        &mut notices,
        ui.pending_deprecated,
        field.pending_deprecated.as_deref(),
    );
    push_notice(&mut notices, ui.deprecated, field.deprecated.as_deref());
    push_notice(
        &mut notices,
        ui.migration,
        field.migration_notice.as_deref(),
    );

    notices
        .into_iter()
        .map(|notice| escape_mdx_text(&notice))
        .collect::<Vec<_>>()
        .join("; ")
}

fn push_notice(notices: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        notices.push(format!("**{label}:** {value}"));
    }
}

fn render_site_index(bundle: &DocBundle, target: &str) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", ui.site_title));
    out.push_str(&format!("- {}: `{target}`\n", ui.target));
    out.push_str(&format!("- {}: `{}`\n", ui.commit, bundle.commit));
    out.push_str(&format!("- {}: `assets/logo.ico`\n\n", ui.asset));

    for language_docs in &bundle.languages {
        out.push_str(&format!("## {}\n\n", language_docs.lang));
        out.push_str(&format!(
            "- [{}]({}/index.md)\n",
            ui.index,
            escape_markdown_link(&language_docs.lang)
        ));
        for app in &language_docs.apps {
            out.push_str(&format!(
                "- [{}]({}/{}.md): {}\n",
                app.name,
                escape_markdown_link(&language_docs.lang),
                escape_markdown_link(&app.name),
                escape_markdown(&app.description)
            ));
        }
        out.push('\n');
    }

    out
}

fn render_language_index(commit: &str, language_docs: &LanguageDocs) -> String {
    let ui = ui_text(&language_docs.lang);
    let mut out = String::new();
    out.push_str(&format!("# {} ({})\n\n", ui.site_title, language_docs.lang));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        ui.app_count,
        language_docs.apps.len()
    ));

    for app in &language_docs.apps {
        out.push_str(&format!(
            "- [{}]({}.md): `{}` {} - {}\n",
            app.name,
            escape_markdown_link(&app.name),
            app.fields.len(),
            ui.field_unit,
            escape_markdown(&app.description)
        ));
    }

    out
}

fn render_mkdocs_config(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("site_name: {}\n", quote_yaml_path(ui.site_title)));
    out.push_str(&format!(
        "site_description: {}\n",
        quote_yaml_path(&format!("{} {}", ui.site_description_prefix, bundle.commit))
    ));
    out.push_str("theme:\n");
    out.push_str("  name: readthedocs\n");
    out.push_str("docs_dir: docs\n");
    out.push_str("nav:\n");
    out.push_str(&format!("  - {}: index.md\n", quote_yaml_key(ui.home)));

    for language_docs in &bundle.languages {
        out.push_str(&format!("  - {}:\n", quote_yaml_key(&language_docs.lang)));
        let language_index = format!("{}/index.md", language_docs.lang);
        out.push_str(&format!(
            "      - {}: {}\n",
            quote_yaml_key(ui_text(&language_docs.lang).index),
            quote_yaml_path(&language_index)
        ));
        for app in &language_docs.apps {
            let app_path = format!("{}/{}.md", language_docs.lang, app.name);
            out.push_str(&format!(
                "      - {}: {}\n",
                quote_yaml_key(&app.name),
                quote_yaml_path(&app_path)
            ));
        }
    }

    out
}

fn render_mdbook_config(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str("[book]\n");
    out.push_str(&format!(
        "title = \"{}\"\n",
        escape_toml_string(ui.site_title)
    ));
    out.push_str("authors = [\"Qexed AutoDoc\"]\n");
    out.push_str("language = \"zh-CN\"\n");
    out.push_str("src = \"src\"\n\n");
    out.push_str("[output.html]\n");
    out.push_str(&format!(
        "git-repository-url = \"https://example.invalid/qexed/{}\"\n",
        escape_toml_string(&bundle.commit)
    ));
    out
}

fn render_mdbook_summary(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", ui.summary));
    out.push_str(&format!("- [{}](index.md)\n", ui.home));

    for language_docs in &bundle.languages {
        out.push_str(&format!(
            "- [{}]({}/index.md)\n",
            language_docs.lang,
            escape_markdown_link(&language_docs.lang)
        ));
        for app in &language_docs.apps {
            out.push_str(&format!(
                "  - [{}]({}/{}.md)\n",
                app.name,
                escape_markdown_link(&language_docs.lang),
                escape_markdown_link(&app.name)
            ));
        }
    }

    out
}

fn render_next_index(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: \"{}\"\n", escape_yaml(ui.site_title)));
    out.push_str(&format!("commit: \"{}\"\n", escape_yaml(&bundle.commit)));
    out.push_str("---\n\n");
    out.push_str(&format!("# {}\n\n", ui.site_title));
    out.push_str(&format!("- {}: `{}`\n", ui.commit, bundle.commit));
    out.push_str(&format!("- {}: `/config/assets/logo.ico`\n\n", ui.asset));

    for language_docs in &bundle.languages {
        out.push_str(&format!("## {}\n\n", language_docs.lang));
        for app in &language_docs.apps {
            out.push_str(&format!(
                "- [{}](./{}/{})\n",
                app.name, language_docs.lang, app.name
            ));
        }
        out.push('\n');
    }

    out
}

fn render_next_app_index(bundle: &DocBundle) -> String {
    let ui = bundle_ui_text(bundle);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", ui.site_title));
    out.push_str(&format!("- {}: `{}`\n\n", ui.commit, bundle.commit));

    for language_docs in &bundle.languages {
        out.push_str(&format!(
            "- [{}](./{})\n",
            language_docs.lang,
            escape_markdown_link(&language_docs.lang)
        ));
    }

    out
}

fn render_next_app_language_index(commit: &str, language_docs: &LanguageDocs) -> String {
    let ui = ui_text(&language_docs.lang);
    let mut out = String::new();
    out.push_str(&format!("# {} - {}\n\n", ui.site_title, language_docs.lang));
    out.push_str(&format!("- {}: `{commit}`\n", ui.commit));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        ui.app_count,
        language_docs.apps.len()
    ));

    for app in &language_docs.apps {
        out.push_str(&format!(
            "- [{}](./{}): `{}` {}\n",
            app.name,
            escape_markdown_link(&app.name),
            app.fields.len(),
            ui.field_unit
        ));
    }

    out
}

fn render_plugin_api_mdx(commit: &str, lang: &str) -> String {
    let zh = is_zh(lang);
    let mut out = String::new();
    out.push_str(&format!(
        "# {}\n",
        text(zh, "插件 API AutoDoc", "Plugin API AutoDoc")
    ));
    out.push_str("> AutoDoc: qexed_config_to_mdx\n\n");
    out.push_str(&format!("- {}: `{}`\n", text(zh, "提交", "Commit"), commit));
    out.push_str(&format!(
        "- {}: `{}`\n",
        text(zh, "ABI", "ABI"),
        "wasm32-unknown-unknown"
    ));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        text(zh, "编码", "Encoding"),
        "postcard"
    ));

    out.push_str(&format!("## {}\n\n", text(zh, "运行时 ABI", "Runtime ABI")));
    out.push_str(&format!(
        "{}\n\n",
        text(
            zh,
            "插件需要导出线性内存和分配函数。除 `qexed_plugin_init()` 外，事件与查询函数都使用 `(ptr: i32, len: i32)` 读取 postcard payload；查询函数返回 `i64`，高 32 位是响应指针，低 32 位是响应长度。",
            "Plugins must export linear memory and an allocator. Except for `qexed_plugin_init()`, event and query functions read postcard payloads through `(ptr: i32, len: i32)`. Query functions return an `i64` where the high 32 bits are the response pointer and the low 32 bits are the response length.",
        )
    ));
    out.push_str("```rust\n");
    out.push_str("qexed_plugin_sdk::qexed_plugin_memory!();\n\n");
    out.push_str("#[unsafe(no_mangle)]\n");
    out.push_str("pub extern \"C\" fn qexed_plugin_priority() -> i32 { 100 }\n");
    out.push_str("```\n\n");
    out.push_str(&format!(
        "- `memory`: {}\n",
        text(zh, "WASM 线性内存导出。", "WASM linear memory export.")
    ));
    out.push_str(&format!(
        "- `qexed_plugin_alloc(len: i32) -> i32`: {}\n",
        text(
            zh,
            "必需，宿主用它写入 payload。",
            "Required; the host uses it to write payloads."
        )
    ));
    out.push_str(&format!(
        "- `qexed_plugin_dealloc(ptr: i32, len: i32)`: {}\n",
        text(
            zh,
            "可选，用于释放宿主写入或插件返回的缓冲区。",
            "Optional; releases buffers written by the host or returned by the plugin."
        )
    ));
    out.push_str(&format!(
        "- `qexed_plugin_priority() -> i32`: {}\n\n",
        text(
            zh,
            "可选，数值越大越先执行。",
            "Optional; larger values run earlier."
        )
    ));

    out.push_str(&format!(
        "## {}\n\n",
        text(zh, "事件与查询", "Events And Queries")
    ));
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n",
        text(zh, "导出函数", "Export"),
        text(zh, "类型", "Kind"),
        "Payload",
        "Response",
        text(zh, "说明", "Description")
    ));
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for event in PLUGIN_EVENT_DOCS {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            event.event.export_name(),
            plugin_call_kind_label(event.kind, zh),
            plugin_type_cell(event.payload),
            plugin_type_cell(event.response),
            escape_markdown_table(localized_plugin_text(zh, event.zh, event.en))
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "## {}\n\n",
        text(zh, "宿主导入 API", "Host Imports")
    ));
    out.push_str(&format!(
        "{}\n\n",
        text(
            zh,
            "这些函数由服务端注册在 `qexed` import module 中；优先通过 `qexed_plugin_sdk` 包装函数调用。",
            "These functions are registered by the server under the `qexed` import module; prefer the wrappers provided by `qexed_plugin_sdk`.",
        )
    ));
    out.push_str(&format!(
        "| {} | {} | {} | {} |\n",
        text(zh, "名称", "Name"),
        text(zh, "宿主签名", "Host Signature"),
        text(zh, "SDK 包装", "SDK Wrapper"),
        text(zh, "说明", "Description")
    ));
    out.push_str("| --- | --- | --- | --- |\n");
    for api in PLUGIN_HOST_API_DOCS {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            api.name,
            escape_code_span(api.signature),
            escape_code_span(api.wrapper),
            escape_markdown_table(localized_plugin_text(zh, api.zh, api.en))
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "## {}\n\n",
        text(zh, "Payload 类型", "Payload Types")
    ));
    for structure in PLUGIN_STRUCT_DOCS {
        out.push_str(&format!("### `{}`\n\n", structure.name));
        out.push_str(localized_plugin_text(zh, structure.zh, structure.en));
        out.push_str("\n\n");
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            text(zh, "字段/变体", "Field/Variant"),
            text(zh, "类型", "Type"),
            text(zh, "默认值", "Default"),
            text(zh, "说明", "Description")
        ));
        out.push_str("| --- | --- | --- | --- |\n");
        for field in structure.fields {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} |\n",
                escape_code_span(field.name),
                escape_code_span(field.ty),
                plugin_default_cell(field.default),
                escape_markdown_table(localized_plugin_text(zh, field.zh, field.en))
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "## {}\n\n",
        text(zh, "最小示例", "Minimal Example")
    ));
    out.push_str("```rust\n");
    out.push_str("use qexed_plugin_sdk::{PlayerPayload, decode_payload};\n\n");
    out.push_str("qexed_plugin_sdk::qexed_plugin_memory!();\n\n");
    out.push_str("#[unsafe(no_mangle)]\n");
    out.push_str("pub extern \"C\" fn qexed_plugin_player_join(ptr: i32, len: i32) {\n");
    out.push_str(
        "    let Some(player) = (unsafe { decode_payload::<PlayerPayload>(ptr, len) }) else {\n",
    );
    out.push_str("        qexed_plugin_sdk::log(\"player_join decode failed\");\n");
    out.push_str("        return;\n");
    out.push_str("    };\n");
    out.push_str("    qexed_plugin_sdk::log(&format!(\"{} joined\", player.username));\n");
    out.push_str("}\n");
    out.push_str("```\n\n");
    out.push_str(&format!(
        "{}\n",
        text(
            zh,
            "更多完整示例见 `plugins/examples`，其中 `command_npc_demo` 覆盖命令、NPC、占位符、自定义实体和插件 AI。",
            "See `plugins/examples` for complete examples. `command_npc_demo` covers commands, NPCs, placeholders, custom entities, and plugin AI.",
        )
    ));

    out
}

fn render_plugin_api_version_index(commit: &str, version_id: &str) -> String {
    format!(
        "# Plugin API AutoDoc\n\n- 提交: `{commit}`\n- 版本: `{version_id}`\n\n- [zh-CN](./zh-CN)\n- [en](./en)\n"
    )
}

fn render_plugin_api_root_index(version_id: &str) -> String {
    format!(
        "# Plugin API AutoDoc\n\n- [稳定版](./stable)\n- [测试版](./beta)\n- [开发版 {version_id}](./dev/{version_id}/zh-CN)\n- [Stable](./stable)\n- [Beta](./beta)\n- [Dev {version_id}](./dev/{version_id}/en)\n"
    )
}

fn render_plugin_api_empty_channel(stable: bool) -> String {
    if stable {
        "# 稳定版 Plugin API AutoDoc\n\nQexed 暂未发布稳定版插件 API 文档。\n".to_string()
    } else {
        "# 测试版 Plugin API AutoDoc\n\nQexed 暂未发布测试版插件 API 文档。\n".to_string()
    }
}

fn render_placeholder_mdx(commit: &str, lang: &str) -> String {
    let zh = is_zh(lang);
    let mut out = String::new();
    out.push_str(&format!(
        "# {}\n\n",
        text(zh, "Placeholder AutoDoc", "Placeholder AutoDoc")
    ));
    out.push_str("> AutoDoc: qexed_config_to_mdx\n\n");
    out.push_str(&format!("- {}: `{}`\n", text(zh, "提交", "Commit"), commit));
    out.push_str(&format!("- {}: `{}`\n", text(zh, "语言", "Language"), lang));
    out.push_str(&format!(
        "- {}: `{}`\n\n",
        text(zh, "占位符数量", "Placeholder count"),
        native_placeholder_docs().len()
    ));

    out.push_str(&format!(
        "{}\n\n",
        text(
            zh,
            "本页列出 Qexed 运行时内置占位符。启用 `qexed_placeholders.toml` 后，计分板、菜单、NPC、广播等服务端文本可使用这些占位符。",
            "This page lists Qexed runtime built-in placeholders. When `qexed_placeholders.toml` is enabled, server-side text such as scoreboards, menus, NPCs, and broadcasts can use these placeholders.",
        )
    ));

    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} |\n",
        text(zh, "占位符", "Placeholder"),
        text(zh, "别名", "Aliases"),
        text(zh, "作用域", "Scope"),
        text(zh, "开始版本", "Since"),
        text(zh, "示例值", "Example"),
        text(zh, "说明", "Description")
    ));
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for doc in native_placeholder_docs() {
        out.push_str(&format!(
            "| `{}` | {} | {} | `{}` | `{}` | {} |\n",
            escape_code_span(doc.token),
            placeholder_alias_cell(doc.aliases),
            placeholder_scope_label(doc.scope, zh),
            escape_code_span(doc.since),
            escape_code_span(doc.example),
            escape_markdown_table(localized_placeholder_text(zh, doc))
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "## {}\n\n",
        text(zh, "插件占位符", "Plugin Placeholders")
    ));
    out.push_str(&format!(
        "{}\n\n",
        text(
            zh,
            "插件可导出 `qexed_plugin_placeholders` 动态返回占位符。返回项的 `key` 可写成 `example` 或 `%example%`；运行时会同时替换 `%example%` 和 `{example}`。",
            "Plugins can export `qexed_plugin_placeholders` to return dynamic placeholders. A returned `key` may be `example` or `%example%`; runtime replaces both `%example%` and `{example}`.",
        )
    ));
    out.push_str("```rust\n");
    out.push_str("use qexed_plugin_sdk::{PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse};\n\n");
    out.push_str("#[unsafe(no_mangle)]\n");
    out.push_str("pub extern \"C\" fn qexed_plugin_placeholders(ptr: i32, len: i32) -> i64 {\n");
    out.push_str("    qexed_plugin_sdk::query(ptr, len, |query: PlaceholderQuery| {\n");
    out.push_str("        PlaceholderResponse {\n");
    out.push_str("            replacements: vec![PlaceholderReplacement {\n");
    out.push_str("                key: \"rank\".to_string(),\n");
    out.push_str("                value: query.player.map(|p| p.username).unwrap_or_default(),\n");
    out.push_str("            }],\n");
    out.push_str("        }\n");
    out.push_str("    })\n");
    out.push_str("}\n");
    out.push_str("```\n");
    out
}

fn render_placeholder_version_index(commit: &str, version_id: &str) -> String {
    format!(
        "# Placeholder AutoDoc\n\n- 提交: `{commit}`\n- 版本: `{version_id}`\n\n- [zh-CN](./zh-CN)\n- [en](./en)\n"
    )
}

fn render_placeholder_root_index(version_id: &str) -> String {
    format!(
        "# Placeholder AutoDoc\n\n- [稳定版](./stable)\n- [测试版](./beta)\n- [开发版 {version_id}](./dev/{version_id}/zh-CN)\n- [Stable](./stable)\n- [Beta](./beta)\n- [Dev {version_id}](./dev/{version_id}/en)\n"
    )
}

fn render_placeholder_empty_channel(stable: bool) -> String {
    if stable {
        "# 稳定版 Placeholder AutoDoc\n\nQexed 暂未发布稳定版占位符文档。\n".to_string()
    } else {
        "# 测试版 Placeholder AutoDoc\n\nQexed 暂未发布测试版占位符文档。\n".to_string()
    }
}

fn placeholder_alias_cell(aliases: &[&str]) -> String {
    if aliases.is_empty() {
        return "-".to_string();
    }
    aliases
        .iter()
        .map(|alias| format!("`{}`", escape_code_span(alias)))
        .collect::<Vec<_>>()
        .join("<br />")
}

fn placeholder_scope_label(scope: PlaceholderScope, zh: bool) -> &'static str {
    match (scope, zh) {
        (PlaceholderScope::Global, true) => "全局",
        (PlaceholderScope::Global, false) => "Global",
        (PlaceholderScope::Lobby, true) => "大厅",
        (PlaceholderScope::Lobby, false) => "Lobby",
        (PlaceholderScope::Player, true) => "玩家",
        (PlaceholderScope::Player, false) => "Player",
    }
}

fn localized_placeholder_text(zh: bool, doc: &PlaceholderDoc) -> &'static str {
    if zh { doc.zh } else { doc.en }
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(12).collect()
}

fn git_source_date() -> Option<String> {
    std::process::Command::new("git")
        .args(["show", "-s", "--format=%cs", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn plugin_call_kind_label(kind: PluginCallKind, zh: bool) -> &'static str {
    match (kind, zh) {
        (PluginCallKind::Event, true) => "事件",
        (PluginCallKind::Event, false) => "Event",
        (PluginCallKind::Query, true) => "查询",
        (PluginCallKind::Query, false) => "Query",
        (PluginCallKind::Hybrid, true) => "事件/查询",
        (PluginCallKind::Hybrid, false) => "Event/Query",
    }
}

fn plugin_type_cell(value: Option<&str>) -> String {
    value
        .map(|value| format!("`{}`", escape_code_span(value)))
        .unwrap_or_else(|| "-".to_string())
}

fn plugin_default_cell(value: &str) -> String {
    if value.is_empty() {
        "-".to_string()
    } else {
        format!("`{}`", escape_code_span(value))
    }
}

fn localized_plugin_text<'a>(zh: bool, zh_text: &'a str, en_text: &'a str) -> &'a str {
    if zh { zh_text } else { en_text }
}

fn text<'a>(zh: bool, zh_text: &'a str, en_text: &'a str) -> &'a str {
    localized_plugin_text(zh, zh_text, en_text)
}

fn is_zh(lang: &str) -> bool {
    lang.to_ascii_lowercase().starts_with("zh")
}

fn escape_yaml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn quote_yaml_key(value: &str) -> String {
    format!("\"{}\"", escape_yaml(value))
}

fn quote_yaml_path(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_markdown_link(value: &str) -> String {
    value
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
}

fn escape_markdown(value: &str) -> String {
    escape_mdx_text(value)
}

fn escape_mdx_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_markdown_table(value: &str) -> String {
    escape_mdx_text(value).replace('|', "\\|")
}

fn escape_code_span(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', " ")
}

fn escape_mdx_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\r', "")
        .replace('\n', "<br />")
}

#[cfg(test)]
mod tests {
    use super::{
        DocApp, DocBundle, DocField, LOGO_BYTES, LanguageDocs, PLUGIN_EVENT_DOCS, Qexed,
        QexedWarden, collect_app, collect_bundle, render_next_app_config_page,
        render_placeholder_mdx, render_plugin_api_mdx, write_assets, write_next_app_docs,
        write_placeholder_next_app_docs, write_plugin_api_next_app_docs,
    };
    use qexed_plugin_api::PluginEvent;

    fn temp_output_dir(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "qexed_config_docs_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time must be after UNIX_EPOCH")
                .as_nanos()
        ));
        path
    }

    #[test]
    fn writes_embedded_logo_asset() -> anyhow::Result<()> {
        let dir = temp_output_dir("embedded_logo");

        write_assets(&dir)?;

        let logo = std::fs::read(dir.join("assets").join("logo.ico"))?;
        assert_eq!(logo, LOGO_BYTES);

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    fn sample_bundle() -> DocBundle {
        let app = DocApp {
            name: "qexed".to_string(),
            config_file: "qexed.toml".to_string(),
            config_path: "config/qexed.toml".to_string(),
            description: "qexed.toml: Qexed 主配置。".to_string(),
            root_path: None,
            fields: vec![
                DocField {
                    path: "server.ip".to_string(),
                    description: "服务器监听地址".to_string(),
                    value_type: "string".to_string(),
                    default_value: Some("\"0.0.0.0:25565\"".to_string()),
                    warning: None,
                    danger: None,
                    pending_deprecated: None,
                    deprecated: None,
                    migration_notice: None,
                },
                DocField {
                    path: "server.motd".to_string(),
                    description: "服务器多行 MOTD\n```python\nprint(\"Hello world\")\n```"
                        .to_string(),
                    value_type: "array".to_string(),
                    default_value: Some("[\"Welcome\", \"to Qexed\"]".to_string()),
                    warning: Some("请避免写入敏感信息。".to_string()),
                    danger: None,
                    pending_deprecated: None,
                    deprecated: None,
                    migration_notice: None,
                },
            ],
        };

        DocBundle {
            commit: "test-commit".to_string(),
            languages: vec![LanguageDocs {
                lang: "zh-CN".to_string(),
                apps: vec![app],
            }],
        }
    }

    #[test]
    fn next_app_page_uses_interactive_config_preview() -> anyhow::Result<()> {
        let bundle = sample_bundle();
        let app = &bundle.languages[0].apps[0];

        let page = render_next_app_config_page(&bundle.commit, &bundle.languages[0], app)?;

        assert!(page.contains("from \"../../ConfigDocClient\""));
        assert!(page.contains("<ConfigDocProvider"));
        assert!(page.contains("<ConfigDocEditor />"));
        assert!(page.contains("<ConfigFieldTable />"));
        assert!(page.contains("<ConfigComplexDetails />"));
        assert!(page.contains("configDocs={configDocs}"));
        assert!(page.contains("\"configPath\": \"config/qexed.toml\""));
        assert!(page.contains("\"description\":"));
        assert!(page.contains("\"currentValue\": \"当前值\""));
        assert!(page.contains("\"path\": \"server.ip\""));
        Ok(())
    }

    #[test]
    fn next_app_writes_client_component_with_copyable_code_blocks() -> anyhow::Result<()> {
        let dir = temp_output_dir("next_app_interactive");
        let bundle = sample_bundle();

        write_next_app_docs(&dir, &bundle)?;

        let docs_dir = dir.join("next-app").join("app").join("docs").join("config");
        let client = std::fs::read_to_string(docs_dir.join("ConfigDocClient.tsx"))?;
        let page = std::fs::read_to_string(docs_dir.join("zh-CN").join("qexed").join("page.mdx"))?;

        assert!(client.contains("export function ConfigDocEditor"));
        assert!(client.contains("export function CopyableCodeBlock"));
        assert!(client.contains("function RichText"));
        assert!(client.contains("splitFencedCode"));
        assert!(client.contains("navigator.clipboard.writeText"));
        assert!(client.contains("qexed-vscode-shell"));
        assert!(client.contains("buildConfigTree"));
        assert!(client.contains("fileNameFromPath(configPath)"));
        assert!(page.contains("```python\\nprint(\\\"Hello world\\\")\\n```"));
        assert!(page.contains("fields={configFields}"));

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn plugin_api_docs_cover_all_plugin_events() {
        for event in PluginEvent::ALL {
            assert!(
                PLUGIN_EVENT_DOCS.iter().any(|doc| doc.event == *event),
                "missing plugin autodoc metadata for {event:?}"
            );
        }

        let zh = render_plugin_api_mdx("test-commit", "zh-CN");
        assert!(zh.contains("# 插件 API AutoDoc"));
        assert!(zh.contains("qexed_plugin_custom_entities"));
        assert!(zh.contains("qexed_plugin_entity_ai_tick"));
        assert!(zh.contains("CustomEntityDefinition"));
        assert!(zh.contains("EntityAiTickQuery"));
        assert!(zh.contains("NpcInteractPayload"));
        assert!(zh.contains("lottery_roll"));
        assert!(zh.contains("pathfinding_find"));

        let en = render_plugin_api_mdx("test-commit", "en");
        assert!(en.contains("# Plugin API AutoDoc"));
        assert!(en.contains("CustomEntityRegistryResponse"));
        assert!(en.contains("EntityAiOperation"));
    }

    #[test]
    fn placeholder_docs_include_native_and_plugin_sections() {
        let zh = render_placeholder_mdx("test-commit", "zh-CN");
        assert!(zh.contains("%online_players%"));
        assert!(zh.contains("%player_dimension%"));
        assert!(zh.contains("qexed_plugin_placeholders"));

        let en = render_placeholder_mdx("test-commit", "en");
        assert!(en.contains("Current online player count."));
        assert!(en.contains("Plugin Placeholders"));
    }

    #[test]
    fn plugin_api_docs_are_written_for_next_app_router() -> anyhow::Result<()> {
        let dir = temp_output_dir("plugin_api_docs");

        write_plugin_api_next_app_docs(
            &dir,
            "test-commit",
            &["zh-CN".to_string(), "en".to_string()],
        )?;

        let zh = std::fs::read_to_string(dir.join("dev/test-commit/zh-CN/page.mdx"))?;
        let en = std::fs::read_to_string(dir.join("dev/test-commit/en/page.mdx"))?;
        assert!(zh.contains("插件 API AutoDoc"));
        assert!(en.contains("Plugin API AutoDoc"));
        assert!(dir.join("versions.json").exists());
        assert!(dir.join("page.mdx").exists());

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn placeholder_docs_are_written_for_next_app_router() -> anyhow::Result<()> {
        let dir = temp_output_dir("placeholder_docs");

        write_placeholder_next_app_docs(
            &dir,
            "1234567890abcdef",
            &["zh-CN".to_string(), "en".to_string()],
        )?;

        assert!(dir.join("versions.json").exists());
        assert!(dir.join("dev/1234567890ab/zh-CN/page.mdx").exists());
        assert!(dir.join("dev/1234567890ab/en/page.mdx").exists());
        assert!(dir.join("page.mdx").exists());

        let versions = std::fs::read_to_string(dir.join("versions.json"))?;
        assert!(versions.contains("1234567890ab"));

        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }

    #[test]
    fn qexed_doc_fields_include_entities_list_structure() -> anyhow::Result<()> {
        let mut bundle = collect_bundle("test-commit", &["zh-CN".to_string()])?;
        let language_docs = bundle.languages.remove(0);
        let app = language_docs
            .apps
            .iter()
            .find(|app| app.name == "qexed_entity")
            .expect("qexed entity app doc must be generated");

        assert_eq!(app.config_path, "config/qexed_entity.toml");
        assert_eq!(app.root_path.as_deref(), None);
        assert!(app.description.contains("qexed_entity.toml"));
        assert_eq!(field_type(app, "entities"), "object");
        assert_eq!(field_type(app, "entities.list"), "array");
        assert_eq!(field_type(app, "entities.list.id"), "string");
        assert_eq!(field_type(app, "entities.list.kind"), "string");
        assert!(
            !app.fields
                .iter()
                .any(|field| field.path == "entities.list.main_hand_event"),
            "NPC click events belong to qexed_npc docs"
        );
        assert_eq!(field_type(app, "entities.list.x"), "number");
        assert_eq!(field_type(app, "entities.list.data"), "integer");
        assert_eq!(field_type(app, "entities.list.on_ground"), "boolean");
        assert!(
            !app.fields
                .iter()
                .any(|field| field.path.starts_with("server.entities")),
            "split docs must use local field paths"
        );
        assert!(
            !app.fields.iter().any(|field| field.value_type == "unknown"),
            "qexed entities docs still contain unknown value types"
        );

        let main = collect_app::<Qexed>("qexed", "qexed.toml", "zh-CN")?;
        assert_eq!(field_type(&main, "version"), "integer");
        assert!(
            !main
                .fields
                .iter()
                .any(|field| field.path.starts_with("server")),
            "qexed main docs must only expose fields kept in qexed.toml"
        );
        Ok(())
    }

    #[test]
    fn qexed_menu_action_split_doc_expands_complex_action() -> anyhow::Result<()> {
        let mut bundle = collect_bundle("test-commit", &["zh-CN".to_string()])?;
        let language_docs = bundle.languages.remove(0);
        let app = language_docs
            .apps
            .iter()
            .find(|app| app.name == "qexed_menus")
            .expect("qexed menus app doc must be generated");

        assert_eq!(app.config_path, "config/qexed_menus.toml");
        assert_eq!(field_type(app, "menus.chests.items.action"), "object");
        assert_eq!(field_type(app, "menus.chests.items.action.kind"), "string");
        assert!(
            !app.fields.iter().any(|field| field.value_type == "unknown"),
            "qexed menus docs still contain unknown value types"
        );
        Ok(())
    }

    #[test]
    fn world_config_is_documented_as_own_file() -> anyhow::Result<()> {
        let bundle = collect_bundle("test-commit", &["zh-CN".to_string()])?;
        let world = bundle.languages[0]
            .apps
            .iter()
            .find(|app| app.name == "world")
            .expect("world config doc must be generated");

        assert_eq!(world.config_path, "config/world.toml");
        assert_eq!(field_type(world, "path"), "string");
        assert_eq!(field_type(world, "gpu.enable"), "boolean");
        assert_eq!(field_type(world, "precompiled_chunks.enable"), "boolean");
        assert!(
            !world
                .fields
                .iter()
                .any(|field| field.value_type == "unknown"),
            "world docs still contain unknown value types"
        );
        Ok(())
    }

    #[test]
    fn proxy_config_is_documented_as_own_file() -> anyhow::Result<()> {
        let bundle = collect_bundle("test-commit", &["zh-CN".to_string()])?;
        let proxy = bundle.languages[0]
            .apps
            .iter()
            .find(|app| app.name == "proxy")
            .expect("proxy config doc must be generated");

        assert_eq!(proxy.config_path, "config/proxy.toml");
        assert_eq!(field_type(proxy, "proxy.enable"), "boolean");
        assert_eq!(field_type(proxy, "proxy.protocol"), "string");
        assert_eq!(field_type(proxy, "proxy.online_mode"), "boolean");
        assert!(proxy.description.contains("proxy.toml"));
        Ok(())
    }

    fn field_type<'a>(app: &'a DocApp, path: &str) -> &'a str {
        app.fields
            .iter()
            .find(|field| field.path == path)
            .unwrap_or_else(|| panic!("missing doc field: {path}"))
            .value_type
            .as_str()
    }

    #[test]
    fn optional_defaults_keep_declared_value_type() -> anyhow::Result<()> {
        let app = collect_app::<QexedWarden>("qexed_warden", "qexed_warden.toml", "zh-CN")?;
        assert_eq!(field_type(&app, "data.pika.master_name"), "string");
        Ok(())
    }
}
