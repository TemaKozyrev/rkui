import { Dialog, DialogContent, DialogHeader, DialogTitle } from "./ui/dialog";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Copy, FileText } from "lucide-react";
import { ScrollArea } from "./ui/scroll-area";
import { Separator } from "./ui/separator";

// Lightweight JSON syntax highlighter without extra deps
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function highlightJson(json: string): string {
  // Tokenize on the raw JSON, escape only the parts we inject to keep XSS-safe
  const regex = /(\"(?:\\.|[^"\\])*\")(\s*:)?|\b(true|false|null)\b|-?\d+(?:\.\d+)?(?:[eE][+\-]?\d+)?/g;
  return json.replace(regex, (match, strWithQuotes, isKeyColon, boolOrNull) => {
    if (strWithQuotes) {
      const safeStr = escapeHtml(strWithQuotes);
      const safeColon = isKeyColon ? escapeHtml(isKeyColon) : "";
      if (isKeyColon) {
        // key
        return `<span class="text-sky-600 dark:text-sky-400">${safeStr}</span>${safeColon}`;
      }
      // string value
      return `<span class="text-emerald-700 dark:text-emerald-400">${safeStr}</span>`;
    }
    if (boolOrNull) {
      const safeBool = escapeHtml(boolOrNull);
      const isNull = boolOrNull === "null";
      const colorClass = isNull ? "text-purple-600 dark:text-purple-400" : "text-orange-600 dark:text-orange-400";
      return `<span class="${colorClass}">${safeBool}</span>`;
    }
    // number
    const safeNum = escapeHtml(match);
    return `<span class="text-amber-600 dark:text-amber-400">${safeNum}</span>`;
  });
}

interface KafkaMessage {
  id: string;
  partition: number;
  key: string;
  offset: number;
  message: string;
  timestamp: string;
  decoding_error?: string; // backend snake_case
  decodingError?: string;  // camelCase fallback
}

interface MessageDetailModalProps {
  message: KafkaMessage | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  messageType?: 'json' | 'text' | 'protobuf';
}

export function MessageDetailModal({ message, open, onOpenChange, messageType = 'json' }: MessageDetailModalProps) {
  if (!message) return null;

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  const formatMessage = (messageContent: string, type: 'json' | 'text' | 'protobuf') => {
    switch (type) {
      case 'json':
        try {
          let parsed: unknown = JSON.parse(messageContent);
          // Handle double-encoded JSON (e.g., a JSON string containing JSON)
          if (typeof parsed === 'string') {
            try {
              const inner = JSON.parse(parsed);
              parsed = inner;
            } catch {
              // ignore if inner is not valid JSON
            }
          }
          return JSON.stringify(parsed, null, 2);
        } catch (e) {
          return messageContent;
        }
      case 'text':
        return messageContent;
      case 'protobuf':
        // If protobuf payload is in JSON representation, pretty-print it as JSON too
        try {
          let parsed: unknown = JSON.parse(messageContent);
          if (typeof parsed === 'string') {
            try {
              const inner = JSON.parse(parsed);
              parsed = inner;
            } catch {
              // not double-encoded
            }
          }
          return JSON.stringify(parsed, null, 2);
        } catch {
          return messageContent;
        }
      default:
        return messageContent;
    }
  };

  const mt: 'json' | 'text' | 'protobuf' = messageType ?? 'json';
  const formattedMessage = formatMessage(message.message, mt);
  const decodingError = (message.decoding_error || message.decodingError) as string | undefined;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[50rem] max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FileText className="h-5 w-5" />
            Message Details
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4">
          {/* Decoding Error (if any) */}
          {decodingError && (
            <div className="p-3 rounded-md border border-red-200 bg-red-50 text-red-700 text-sm">
              <div className="font-semibold mb-1">Ошибка декодирования</div>
              <div className="whitespace-pre-wrap break-words">{decodingError}</div>
            </div>
          )}

          {/* Message Metadata */}
          <div className="grid grid-cols-2 gap-4 p-4 bg-muted/30 rounded-lg md:grid-cols-[10%_15%_35%_40%] overflow-x-auto">
            <div className="min-w-0">
              <div className="text-sm text-muted-foreground">Partition</div>
              <Badge variant="outline">{message.partition}</Badge>
            </div>
            <div className="min-w-0">
              <div className="text-sm text-muted-foreground">Offset</div>
              <div className="font-mono text-sm truncate">{message.offset}</div>
            </div>
            <div className="min-w-0">
              <div className="text-sm text-muted-foreground">Key</div>
              <div className="font-mono text-sm truncate">{message.key}</div>
            </div>
            <div className="min-w-0">
              <div className="text-sm text-muted-foreground">Timestamp</div>
              <div className="text-sm truncate">{message.timestamp}</div>
            </div>
          </div>

          <Separator />

          {/* Message Content */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <h3>Message Content</h3>
              <div className="flex gap-2">
                <Badge variant="secondary">{mt.toUpperCase()}</Badge>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => copyToClipboard(formattedMessage)}
                  className="gap-2"
                >
                  <Copy className="h-4 w-4" />
                  Copy
                </Button>
              </div>
            </div>
            
            <ScrollArea className="h-96 w-full border rounded-md">
              <div className="p-4">
                {mt === 'json' ? (
                  <pre className="text-sm font-mono whitespace-pre-wrap break-all">
                    <code dangerouslySetInnerHTML={{ __html: highlightJson(formattedMessage) }} />
                  </pre>
                ) : mt === 'protobuf' ? (
                  // If protobuf content is actually JSON, render with JSON highlighting too
                  (() => {
                    const looksLikeJson = (() => {
                      const t = formattedMessage.trim();
                      if (!t) return false;
                      if (t.startsWith('{') || t.startsWith('[') || t.startsWith('"')) {
                        try { JSON.parse(t); return true; } catch { return false; }
                      }
                      return false;
                    })();
                    if (looksLikeJson) {
                      return (
                        <div>
                          <pre className="text-sm font-mono whitespace-pre-wrap break-all">
                            <code dangerouslySetInnerHTML={{ __html: highlightJson(formattedMessage) }} />
                          </pre>
                          <div className="mt-2 p-2 bg-muted/50 rounded text-xs text-muted-foreground">
                            Showing protobuf message in its JSON representation.
                          </div>
                        </div>
                      );
                    }
                    return (
                      <div className="text-sm font-mono text-muted-foreground">
                        {formattedMessage}
                        <div className="mt-2 p-2 bg-muted/50 rounded text-xs">
                          Note: Protobuf messages are displayed as binary data. 
                          Configure a proto schema to view decoded content.
                        </div>
                      </div>
                    );
                  })()
                ) : (
                  <div className="text-sm font-mono whitespace-pre-wrap break-all">
                    {formattedMessage}
                  </div>
                )}
              </div>
            </ScrollArea>
          </div>

          {/* Additional Message Info */}
          <div className="p-4 bg-muted/30 rounded-lg space-y-2">
            <h4>Raw Message Info</h4>
            <div className="grid grid-cols-1 gap-2 text-sm">
              <div>
                <span className="text-muted-foreground">Message ID:</span>
                <span className="ml-2 font-mono">{message.id}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Size:</span>
                <span className="ml-2">{new Blob([message.message]).size} bytes</span>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}