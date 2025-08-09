import { useState, useEffect, useMemo } from "react";
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog, message as showDialog } from '@tauri-apps/plugin-dialog';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Switch } from "./ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Settings, Upload, FileText } from "lucide-react";

interface ConfigurationData {
  broker: string;
  topic: string;
  sslEnabled: boolean;
  sslCertPath?: string;
  sslKeyPath?: string;
  sslCaPath?: string;
  messageType: 'json' | 'text' | 'protobuf';
  // Legacy single proto schema path (kept for backward compatibility with backend)
  protoSchemaPath?: string;
  // New fields for proto mode (UI only for now)
  protoFiles?: string[];
  protoSelectedMessage?: string;
}

interface ConfigurationModalProps {
  onConfigurationSave: (config: ConfigurationData) => void;
}

export function ConfigurationModal({ onConfigurationSave }: ConfigurationModalProps) {
  const [open, setOpen] = useState(false);
  const [config, setConfig] = useState<ConfigurationData>({
    broker: 'localhost:9092',
    topic: 'user-events',
    sslEnabled: false,
    messageType: 'json'
  });
  const [topics, setTopics] = useState<string[]>([]);
  const [topicsBroker, setTopicsBroker] = useState<string | null>(null);
  const [isFetchingTopics, setIsFetchingTopics] = useState(false);
  const [topicsError, setTopicsError] = useState<string | null>(null);
  const [topicFocused, setTopicFocused] = useState(false);

  // Proto parsing UI state
  const [protoFiles, setProtoFiles] = useState<string[]>([]);
  const [isParsingProto, setIsParsingProto] = useState(false);
  const [parsedMessages, setParsedMessages] = useState<string[]>([]);
  const [parseError, setParseError] = useState<string | null>(null);

  const handlePickProtoFile = async () => {
    try {
      const selected = await openDialog({ filters: [{ name: 'Proto', extensions: ['proto'] }], multiple: true });
      if (typeof selected === 'string') {
        setProtoFiles([selected]);
        setConfig(prev => ({ ...prev, protoSchemaPath: selected, protoFiles: [selected] }));
      } else if (Array.isArray(selected)) {
        const files = selected.filter((x): x is string => typeof x === 'string');
        setProtoFiles(files);
        setConfig(prev => ({ ...prev, protoSchemaPath: files[0], protoFiles: files }));
      }
    } catch (e: any) {
      console.error('Failed to pick proto file(s)', e);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to open file dialog');
      await showDialog(text, { title: 'Error', kind: 'error' });
      // Also reflect inline for context
      setParseError('Failed to open file dialog. Please run inside Tauri and ensure dialog permissions are enabled. See console for details.');
    }
  };

  const handleLoadProtoMetadata = async () => {
    if (protoFiles.length === 0) return;
    try {
      setIsParsingProto(true);
      setParseError(null);
      const res = await invoke<{ packages: string[]; messages: string[] }>("parse_proto_metadata", { files: protoFiles });
      setParsedMessages(res?.messages || []);
      // If previously selected message is not in new list, clear it
      setConfig(prev => ({
        ...prev,
        protoSelectedMessage: prev.protoSelectedMessage && (res?.messages || []).includes(prev.protoSelectedMessage)
          ? prev.protoSelectedMessage
          : undefined,
      }));
    } catch (e: any) {
      console.error('Failed to parse proto metadata', e);
      setParsedMessages([]);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to parse proto metadata');
      setParseError(text);
      await showDialog(text, { title: 'Proto Error', kind: 'error' });
    } finally {
      setIsParsingProto(false);
    }
  };

  const handleSave = () => {
    onConfigurationSave(config);
    setOpen(false);
  };

  const toBackendConfig = (c: ConfigurationData) => ({
    broker: c.broker,
    topic: c.topic,
    ssl_enabled: c.sslEnabled,
    ssl_cert_path: c.sslCertPath || null,
    ssl_key_path: c.sslKeyPath || null,
    ssl_ca_path: c.sslCaPath || null,
    message_type: c.messageType,
    proto_schema_path: c.protoSchemaPath || null,
    proto_message_full_name: c.protoSelectedMessage || null,
  });

  const fetchTopicsForBroker = async (broker: string) => {
    const trimmed = broker.trim();
    if (!trimmed) return;
    if (topicsBroker === trimmed) return; // already fetched for this broker
    try {
      setIsFetchingTopics(true);
      setTopicsError(null);
      const payload = toBackendConfig({ ...config, broker: trimmed });
      const list = await invoke<string[]>("get_topics", { config: payload });
      setTopics(list || []);
      setTopicsBroker(trimmed);
    } catch (e: any) {
      console.error("Failed to fetch topics:", e);
      setTopics([]);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to fetch topics');
      setTopicsError(text);
      setTopicsBroker(null);
      await showDialog(text, { title: 'Topics Error', kind: 'error' });
    } finally {
      setIsFetchingTopics(false);
    }
  };

  const handleBrokerBlur = () => {
    void fetchTopicsForBroker(config.broker);
  };

  const filteredTopics = useMemo(() => {
    const q = (config.topic || '').toLowerCase();
    if (!q) return topics;
    return topics.filter(t => t.toLowerCase().includes(q));
  }, [config.topic, topics]);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" className="gap-2">
          <Settings className="h-4 w-4" />
          Configure
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-4xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Kafka Configuration</DialogTitle>
        </DialogHeader>
        
        <Tabs defaultValue="connection" className="w-full">
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="connection">Connection</TabsTrigger>
            <TabsTrigger value="ssl">SSL</TabsTrigger>
            <TabsTrigger value="messages">Messages</TabsTrigger>
          </TabsList>
          
          <TabsContent value="connection" className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>Broker Settings</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div>
                  <Label htmlFor="broker">Kafka Broker</Label>
                  <Input
                    id="broker"
                    value={config.broker}
                    onChange={(e) => {
                      const val = e.target.value;
                      setConfig(prev => ({ ...prev, broker: val }));
                      if (topicsBroker && val !== topicsBroker) {
                        setTopics([]);
                        setTopicsBroker(null);
                        setTopicsError(null);
                      }
                    }}
                    onBlur={handleBrokerBlur}
                    placeholder="localhost:9092"
                  />
                </div>
                <div className="relative">
                  <Label htmlFor="topic">Topic</Label>
                  <Input
                    id="topic"
                    value={config.topic}
                    onFocus={() => setTopicFocused(true)}
                    onBlur={() => setTimeout(() => setTopicFocused(false), 100)}
                    onChange={(e) => setConfig(prev => ({ ...prev, topic: e.target.value }))}
                    placeholder="my-topic"
                    autoComplete="off"
                  />
                  {(topicFocused && (isFetchingTopics || topicsError || topics.length > 0)) && (
                    <div className="absolute z-50 mt-1 w-full max-h-60 overflow-auto rounded-md border bg-background shadow-md">
                      {isFetchingTopics && (
                        <div className="px-3 py-2 text-sm text-muted-foreground">Loading topics…</div>
                      )}
                      {!isFetchingTopics && topicsError && (
                        <div className="px-3 py-2 text-sm text-destructive">{topicsError}</div>
                      )}
                      {!isFetchingTopics && !topicsError && filteredTopics.map((t) => (
                        <div
                          key={t}
                          className="px-3 py-2 cursor-pointer hover:bg-accent hover:text-accent-foreground text-sm"
                          onMouseDown={() => {
                            setConfig(prev => ({ ...prev, topic: t }));
                            setTopicFocused(false);
                          }}
                        >
                          {t}
                        </div>
                      ))}
                      {!isFetchingTopics && !topicsError && filteredTopics.length === 0 && (
                        <div className="px-3 py-2 text-sm text-muted-foreground">No matching topics</div>
                      )}
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          </TabsContent>
          
          <TabsContent value="ssl" className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>SSL Configuration</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="flex items-center space-x-2">
                  <Switch
                    id="ssl-enabled"
                    checked={config.sslEnabled}
                    onCheckedChange={(checked: boolean) => setConfig(prev => ({ ...prev, sslEnabled: checked }))}
                  />
                  <Label htmlFor="ssl-enabled">Enable SSL</Label>
                </div>
                
                {config.sslEnabled && (
                  <>
                    <div>
                      <Label htmlFor="ssl-cert">Certificate Path</Label>
                      <Input
                        id="ssl-cert"
                        value={config.sslCertPath || ''}
                        onChange={(e) => setConfig(prev => ({ ...prev, sslCertPath: e.target.value }))}
                        placeholder="/path/to/client.crt"
                      />
                    </div>
                    <div>
                      <Label htmlFor="ssl-key">Key Path</Label>
                      <Input
                        id="ssl-key"
                        value={config.sslKeyPath || ''}
                        onChange={(e) => setConfig(prev => ({ ...prev, sslKeyPath: e.target.value }))}
                        placeholder="/path/to/client.key"
                      />
                    </div>
                    <div>
                      <Label htmlFor="ssl-ca">CA Certificate Path</Label>
                      <Input
                        id="ssl-ca"
                        value={config.sslCaPath || ''}
                        onChange={(e) => setConfig(prev => ({ ...prev, sslCaPath: e.target.value }))}
                        placeholder="/path/to/ca.crt"
                      />
                    </div>
                  </>
                )}
              </CardContent>
            </Card>
          </TabsContent>
          
          <TabsContent value="messages" className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>Message Format</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div>
                  <Label htmlFor="message-type">Message Type</Label>
                  <Select
                    value={config.messageType}
                    onValueChange={(value: 'json' | 'text' | 'protobuf') => 
                      setConfig(prev => ({ ...prev, messageType: value }))
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
                      {isParsingProto && (
                        <div className="text-sm text-muted-foreground">Parsing schemas…</div>
                      )}
                      {!isParsingProto && parseError && (
                        <div className="text-sm text-destructive">{parseError}</div>
                      )}
                    </div>

                    {!isParsingProto && parsedMessages.length > 0 && (
                      <div>
                        <Label htmlFor="proto-message">Select Message Type</Label>
                        <Select
                          value={config.protoSelectedMessage || ''}
                          onValueChange={(value: string) => setConfig(prev => ({ ...prev, protoSelectedMessage: value }))}
                        >
                          <SelectTrigger id="proto-message">
                            <SelectValue placeholder="Choose message type" />
                          </SelectTrigger>
                          <SelectContent>
                            {parsedMessages.map(m => (
                              <SelectItem key={m} value={m}>{m}</SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                    )}
                  </div>
                )}


              </CardContent>
            </Card>
          </TabsContent>
          

        </Tabs>
        
        <div className="flex justify-end space-x-2 pt-4">
          <Button variant="outline" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave}>
            Save Configuration
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}