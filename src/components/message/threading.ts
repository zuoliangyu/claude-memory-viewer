import type { DisplayMessage } from "../../types";

export interface ThreadDisplayNode {
  id: string;
  originalIndex: number;
  message: DisplayMessage;
  children: ThreadDisplayNode[];
  sectionPath: number[];
  sectionLabel: string;
  threadTitle: string;
  threadAnchor: string | null;
  mentionAnchors: string[];
  parentSource: "parent_uuid" | "mention" | null;
  forkUserMessageId: string | null;
}

interface BuiltMessageTree {
  roots: ThreadDisplayNode[];
  isThreaded: boolean;
}

const THREAD_ANCHOR_LENGTH = 8;
const THREAD_TOKEN_MAX_LENGTH = 32;
const THREAD_MENTION_REGEX = /(^|[\s([{\u3000\uFF08\u3010])@([a-z0-9-]{4,32})\b/gi;
const USER_TITLE_LIMIT = 80;
const ASSISTANT_TITLE_LIMIT = 64;
const TOOL_TITLE_LIMIT = 56;
const SENTENCE_END_REGEX = /[。！？!?；;…]/;
const DOT_SENTENCE_REGEX = /\.(?=\s|$)/;

export function getMessageKey(message: DisplayMessage, index: number) {
  return message.uuid || `${message.role}-${index}`;
}

export function getUserMessageId(message: DisplayMessage, index: number) {
  return message.uuid || `user-${index}`;
}

export function buildMessageTree(messages: DisplayMessage[]): BuiltMessageTree {
  const nodes = messages.map<ThreadDisplayNode>((message, originalIndex) => ({
    id: getMessageKey(message, originalIndex),
    originalIndex,
    message,
    children: [],
    sectionPath: [],
    sectionLabel: "",
    threadTitle: "",
    threadAnchor: getThreadAnchor(message),
    mentionAnchors: extractThreadMentions(message),
    parentSource: null,
    forkUserMessageId: null,
  }));

  const uuidMap = new Map<string, ThreadDisplayNode>();
  const assistantMentionLookup = new Map<string, ThreadDisplayNode | null>();

  for (const node of nodes) {
    if (node.message.uuid) {
      uuidMap.set(node.message.uuid, node);
    }
    if (node.message.role === "assistant" && node.threadAnchor) {
      registerAssistantMentionPrefixes(node, assistantMentionLookup);
    }
  }

  const roots: ThreadDisplayNode[] = [];
  let linkedCount = 0;

  for (const node of nodes) {
    let parent: ThreadDisplayNode | null = null;
    let parentSource: ThreadDisplayNode["parentSource"] = null;

    const explicitParentUuid = node.message.parentUuid;
    if (explicitParentUuid) {
      const explicitParent = uuidMap.get(explicitParentUuid);
      if (explicitParent && explicitParent !== node && explicitParent.originalIndex < node.originalIndex) {
        parent = explicitParent;
        parentSource = "parent_uuid";
      }
    }

    if (!parent && node.message.role === "user") {
      for (const mention of node.mentionAnchors) {
        const mentionedAssistant = assistantMentionLookup.get(normalizeThreadToken(mention));
        if (
          mentionedAssistant &&
          mentionedAssistant !== node &&
          mentionedAssistant.originalIndex < node.originalIndex
        ) {
          parent = mentionedAssistant;
          parentSource = "mention";
          break;
        }
      }
    }

    if (node.message.role === "assistant" && explicitParentUuid) {
      node.forkUserMessageId = resolveForkUserMessageId(node, uuidMap, explicitParentUuid);
    }

    node.parentSource = parentSource;

    if (parent) {
      parent.children.push(node);
      linkedCount += 1;
    } else {
      roots.push(node);
    }
  }

  assignSectionLabels(roots);
  assignThreadTitles(roots);

  return {
    roots,
    isThreaded: linkedCount > 0,
  };
}

export function getThreadAnchor(message: DisplayMessage): string | null {
  if (!message.uuid) {
    return null;
  }

  return `@${normalizeUuidPrefix(message.uuid)}`;
}

export function extractThreadMentions(message: DisplayMessage): string[] {
  const text = message.content
    .filter((block): block is { type: "text"; text: string } => block.type === "text")
    .map((block) => block.text)
    .join("\n");

  if (!text) {
    return [];
  }

  const mentions: string[] = [];
  const seen = new Set<string>();

  for (const match of text.matchAll(THREAD_MENTION_REGEX)) {
    const normalized = `@${normalizeThreadToken(match[2] || "")}`;
    if (normalized === "@" || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    mentions.push(normalized);
  }

  return mentions;
}

function normalizeUuidPrefix(value: string): string {
  return value.replace(/-/g, "").toLowerCase().slice(0, THREAD_ANCHOR_LENGTH);
}

function normalizeThreadToken(value: string): string {
  return value.replace(/^@/, "").replace(/-/g, "").toLowerCase().slice(0, THREAD_TOKEN_MAX_LENGTH);
}

function registerAssistantMentionPrefixes(
  node: ThreadDisplayNode,
  lookup: Map<string, ThreadDisplayNode | null>
) {
  const uuid = node.message.uuid;
  if (!uuid) {
    return;
  }

  const normalizedUuid = normalizeThreadToken(uuid);
  for (let length = 4; length <= normalizedUuid.length; length += 1) {
    const prefix = normalizedUuid.slice(0, length);
    const existing = lookup.get(prefix);
    if (!existing) {
      lookup.set(prefix, node);
      continue;
    }
    if (existing !== node) {
      lookup.set(prefix, null);
    }
  }
}

function resolveForkUserMessageId(
  node: ThreadDisplayNode,
  uuidMap: Map<string, ThreadDisplayNode>,
  explicitParentUuid: string
): string | null {
  const explicitParent = uuidMap.get(explicitParentUuid);
  if (
    !explicitParent ||
    explicitParent === node ||
    explicitParent.originalIndex >= node.originalIndex ||
    explicitParent.message.role !== "user"
  ) {
    return null;
  }

  return explicitParentUuid;
}

function assignSectionLabels(roots: ThreadDisplayNode[]) {
  const stack = roots
    .map((node, index) => ({ node, sectionPath: [index + 1] }))
    .reverse();

  while (stack.length > 0) {
    const current = stack.pop();
    if (!current) continue;
    current.node.sectionPath = current.sectionPath;
    current.node.sectionLabel = current.sectionPath.join(".");

    for (let index = current.node.children.length - 1; index >= 0; index -= 1) {
      stack.push({
        node: current.node.children[index],
        sectionPath: [...current.sectionPath, index + 1],
      });
    }
  }
}

function assignThreadTitles(roots: ThreadDisplayNode[]) {
  const stack = roots
    .map((node) => ({ node, parent: null as ThreadDisplayNode | null }))
    .reverse();

  while (stack.length > 0) {
    const current = stack.pop();
    if (!current) continue;
    current.node.threadTitle = deriveThreadTitle(current.node, current.parent);

    for (let index = current.node.children.length - 1; index >= 0; index -= 1) {
      stack.push({ node: current.node.children[index], parent: current.node });
    }
  }
}

function deriveThreadTitle(node: ThreadDisplayNode, parent: ThreadDisplayNode | null): string {
  switch (node.message.role) {
    case "user":
      return deriveUserTitle(node.message);
    case "assistant":
      return deriveAssistantTitle(node.message);
    case "tool":
      return deriveToolTitle(node.message, parent);
    default:
      return "消息";
  }
}

function deriveUserTitle(message: DisplayMessage): string {
  const text = cleanMessageText(getTextBlocks(message, ["text"]));
  return limitText(text || "（用户消息）", USER_TITLE_LIMIT);
}

function deriveAssistantTitle(message: DisplayMessage): string {
  const text = cleanMessageText(getTextBlocks(message, ["text", "reasoning", "thinking"]));
  if (text) {
    return limitText(extractFirstSentence(text), ASSISTANT_TITLE_LIMIT);
  }

  const actionTitle = deriveActionTitleFromBlocks(message.content);
  if (actionTitle) {
    return limitText(actionTitle, TOOL_TITLE_LIMIT);
  }

  return "cc回答";
}

function deriveToolTitle(message: DisplayMessage, parent: ThreadDisplayNode | null): string {
  const parentAction = parent ? deriveActionTitleFromBlocks(parent.message.content) : "";
  if (parentAction) {
    return limitText(parentAction, TOOL_TITLE_LIMIT);
  }

  const toolText = cleanMessageText(getTextBlocks(message, ["tool_result", "function_call_output"]));
  if (toolText) {
    return limitText(extractFirstSentence(toolText), TOOL_TITLE_LIMIT);
  }

  return "工具输出";
}

function deriveActionTitleFromBlocks(content: DisplayMessage["content"]): string {
  for (const block of content) {
    if (block.type === "tool_use") {
      return deriveToolUseTitle(block.name, block.input);
    }
    if (block.type === "function_call") {
      return deriveToolUseTitle(block.name, block.arguments);
    }
  }
  return "";
}

function deriveToolUseTitle(name: string, rawInput: string): string {
  const normalized = normalizeActionName(name);
  const parsedInput = tryParseJson(rawInput);
  const filePath = extractFilePath(parsedInput);
  const fileName = filePath ? getFileName(filePath) : "";
  const command = extractCommand(parsedInput, rawInput);

  if (isReadAction(normalized) && fileName) {
    return `read ${fileName}`;
  }
  if (isWriteAction(normalized) && fileName) {
    return `写 ${fileName}`;
  }
  if (isCommandAction(normalized) && command) {
    return `执行 ${limitText(command, 32)}`;
  }
  if (fileName) {
    return `${normalized} ${fileName}`;
  }
  if (command) {
    return `${normalized} ${limitText(command, 32)}`;
  }
  return normalized || "cc回答";
}

function getTextBlocks(
  message: DisplayMessage,
  types: Array<DisplayMessage["content"][number]["type"]>
): string {
  return message.content
    .filter((block) => types.includes(block.type))
    .map((block) => {
      switch (block.type) {
        case "text":
          return block.text;
        case "reasoning":
          return block.text;
        case "thinking":
          return block.thinking;
        case "tool_result":
          return block.content;
        case "function_call_output":
          return block.output;
        default:
          return "";
      }
    })
    .filter(Boolean)
    .join("\n");
}

function cleanMessageText(value: string): string {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && line !== "```")
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

function extractFirstSentence(value: string): string {
  if (!value) {
    return "";
  }

  const chars = Array.from(value);
  for (let i = 0; i < chars.length; i += 1) {
    const char = chars[i];
    if (SENTENCE_END_REGEX.test(char)) {
      return value.slice(0, i + 1).trim();
    }
    if (char === "." && DOT_SENTENCE_REGEX.test(value.slice(i, i + 2))) {
      return value.slice(0, i + 1).trim();
    }
  }

  return value.trim();
}

function limitText(value: string, maxLength: number): string {
  const chars = Array.from(value);
  if (chars.length <= maxLength) {
    return value;
  }
  return `${chars.slice(0, maxLength).join("").trim()}...`;
}

function tryParseJson(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    // Ignore invalid JSON.
  }
  return null;
}

function extractFilePath(input: Record<string, unknown> | null): string {
  if (!input) {
    return "";
  }
  const candidate = input.file_path ?? input.path ?? input.target_file ?? input.filePath;
  return typeof candidate === "string" ? candidate : "";
}

function extractCommand(input: Record<string, unknown> | null, rawInput: string): string {
  if (input?.command && typeof input.command === "string") {
    return input.command.trim();
  }
  return rawInput.trim();
}

function getFileName(path: string): string {
  const normalized = path.split(/[\\/]/);
  return normalized[normalized.length - 1] || path;
}

function normalizeActionName(value: string): string {
  return value.trim().toLowerCase().replace(/[\s_-]+/g, "");
}

function isReadAction(name: string): boolean {
  return name === "read" || name === "readfile";
}

function isWriteAction(name: string): boolean {
  return name === "write" || name === "edit" || name === "writefile";
}

function isCommandAction(name: string): boolean {
  return name === "bash" || name === "shell" || name === "command" || name === "run";
}
