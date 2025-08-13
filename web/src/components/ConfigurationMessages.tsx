import { Card, CardContent } from "./ui/card";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Button } from "./ui/button";
import { Upload, FileText } from "lucide-react";

interface ConfigMessagesShape {
  messageType: 'json' | 'text' | 'protobuf';
  protoSelectedMessage?: string;
}

interface Props {
  config: ConfigMessagesShape;
  setConfig: React.Dispatch<React.SetStateAction<any>>;
  protoFiles: string[];
  isParsingProto: boolean;
  parsedMessages: string[];
  parseError: string | null;
  handlePickProtoFile: () => Promise<void>;
  handleLoadProtoMetadata: () => Promise<void>;
  handleCleanProto: () => void;
}

export default function ConfigurationMessages({
  config,
  setConfig,
  protoFiles,
  isParsingProto,
  parsedMessages,
  parseError,
  handlePickProtoFile,
  handleLoadProtoMetadata,
  handleCleanProto,
}: Props) {
  return (
    <Card>
      <CardContent className="pt-2 space-y-4">
        <div>
          <Label htmlFor="message-type">Message Type</Label>
          <Select
            value={config.messageType}
            onValueChange={(value: 'json' | 'text' | 'protobuf') =>
              setConfig((prev: any) => ({ ...prev, messageType: value }))
            }
          >
            <SelectTrigger>
              <SelectValue placeholder="Select message type" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="json">JSON</SelectItem>
              <SelectItem value="text">Text</SelectItem>
              <SelectItem value="protobuf">Protocol Buffers</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {config.messageType === 'protobuf' && (
          <div className="space-y-3">
            <div>
              <Label>Proto Schema Files</Label>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  onClick={handlePickProtoFile}
                  className="gap-2"
                >
                  <Upload className="h-4 w-4" />
                  Select .proto files
                </Button>
                {protoFiles.length > 0 && (
                  <div className="text-xs text-muted-foreground">
                    {protoFiles.length} file(s) selected
                  </div>
                )}
              </div>
              {protoFiles.length > 0 && (
                <div className="mt-2 max-h-24 overflow-auto rounded border p-2 text-xs">
                  {protoFiles.map((f) => (
                    <div key={f} className="flex items-center gap-1">
                      <FileText className="h-3 w-3" />
                      <span className="truncate">{f}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="flex items-center gap-2">
              <Button onClick={handleLoadProtoMetadata} disabled={protoFiles.length === 0 || isParsingProto}>
                {isParsingProto ? 'Loading…' : 'Load'}
              </Button>
              <Button variant="outline" onClick={handleCleanProto} disabled={isParsingProto || (protoFiles.length === 0 && parsedMessages.length === 0 && !config.protoSelectedMessage)}>
                Clean
              </Button>
              {isParsingProto && (
                <div className="text-sm text-muted-foreground">Parsing schemas…</div>
              )}
              {!isParsingProto && parseError && (
                <div className="text-sm text-destructive">{parseError}</div>
              )}
            </div>

            <div>
              <Label htmlFor="proto-message">Select Message Type</Label>
              <Select
                value={config.protoSelectedMessage || ''}
                onValueChange={(value: string) => setConfig((prev: any) => ({ ...prev, protoSelectedMessage: value }))}
              >
                <SelectTrigger id="proto-message" disabled={isParsingProto || parsedMessages.length === 0}>
                  <SelectValue placeholder={parsedMessages.length === 0 ? "Load schemas to choose" : "Choose message type"} />
                </SelectTrigger>
                <SelectContent>
                  {parsedMessages.map(m => (
                    <SelectItem key={m} value={m}>{m}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
