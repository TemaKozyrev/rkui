import { useState, useEffect, useMemo } from "react";
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog, message as showDialog } from '@tauri-apps/plugin-dialog';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Settings, Upload, FileText } from "lucide-react";

interface ConfigurationData {
  name?: string; // UI-only: human-friendly name for saved configs
  broker: string;
  topic: string;
  securityType: 'plaintext' | 'ssl' | 'sasl_plaintext';
  sslCertPath?: string;
  sslKeyPath?: string;
  sslCaPath?: string;
  saslMechanism?: string;
  saslJaasConfig?: string;
  messageType: 'json' | 'text' | 'protobuf';
  // Legacy single proto schema path (kept for backward compatibility with backend)
  protoSchemaPath?: string;
  // New fields for proto mode (UI only for now)
  protoFiles?: string[];
  protoSelectedMessage?: string;
  // Backward compat flag (unused in new UI)
  sslEnabled?: boolean;
  // Cache key returned by backend when proto files are parsed
  protoDescriptorKey?: string;
}

interface ConfigurationModalProps {
  onConfigurationSave: (config: ConfigurationData) => void;
}

export function ConfigurationModal({ onConfigurationSave }: ConfigurationModalProps) {
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<'list' | 'edit'>('list');
  const [config, setConfig] = useState<ConfigurationData>({
    name: '',
    broker: 'localhost:9092',
    topic: 'user-events',
    securityType: 'plaintext',
    messageType: 'json'
  });
  const [savedConfigs, setSavedConfigs] = useState<ConfigurationData[]>([]);
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
      let incoming: string[] = [];
      if (typeof selected === 'string') {
        incoming = [selected];
      } else if (Array.isArray(selected)) {
        incoming = selected.filter((x): x is string => typeof x === 'string');
      } else {
        return;
      }
      if (incoming.length === 0) return;
      // Merge with already selected files and skip duplicates
      const mergedSet = new Set<string>(protoFiles);
      for (const f of incoming) {
        mergedSet.add(f);
      }
      const merged = Array.from(mergedSet);
      setProtoFiles(merged);
      setConfig(prev => {
        const nextProtoSchemaPath = prev.protoSchemaPath || merged[0];
        return { ...prev, protoSchemaPath: nextProtoSchemaPath, protoFiles: merged };
      });
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
      const res = await invoke<{ packages: string[]; messages: string[]; cache_key?: string; cacheKey?: string }>("parse_proto_metadata", { files: protoFiles });
      setParsedMessages(res?.messages || []);
      const key = (res as any)?.cache_key || (res as any)?.cacheKey;
      // If previously selected message is not in new list, clear it; store descriptor cache key if provided
      setConfig(prev => ({
        ...prev,
        protoSelectedMessage: prev.protoSelectedMessage && (res?.messages || []).includes(prev.protoSelectedMessage)
          ? prev.protoSelectedMessage
          : undefined,
        protoDescriptorKey: typeof key === 'string' && key ? key : prev.protoDescriptorKey,
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

  const handleCleanProto = () => {
    setProtoFiles([]);
    setParsedMessages([]);
    setParseError(null);
    setConfig(prev => ({ ...prev, protoSchemaPath: undefined, protoFiles: [], protoSelectedMessage: undefined, protoDescriptorKey: undefined }));
  };

  // Storage helpers
  const STORAGE_KEY = 'rkui.savedConfigs';
  const loadSaved = (): ConfigurationData[] => {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return [];
      const arr = JSON.parse(raw);
      if (!Array.isArray(arr)) return [];
      return arr as ConfigurationData[];
    } catch {
      return [];
    }
  };
  const persistSaved = (arr: ConfigurationData[]) => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(arr));
  };

  useEffect(() => {
    if (open) {
      const list = loadSaved();
      setSavedConfigs(list);
      setMode('list');
    }
  }, [open]);

  const startNewConfig = () => {
    setConfig({
      name: '',
      broker: 'localhost:9092',
      topic: 'user-events',
      securityType: 'plaintext',
      messageType: 'json',
      protoFiles: [],
      protoSelectedMessage: undefined,
      protoSchemaPath: undefined,
    });
    setProtoFiles([]);
    setParsedMessages([]);
    setParseError(null);
    setMode('edit');
  };

  const deleteConfig = (name: string) => {
    const next = savedConfigs.filter(c => (c.name || deriveName(c)) !== name);
    setSavedConfigs(next);
    persistSaved(next);
  };

  const deriveName = (c: ConfigurationData) => {
    const base = c.name && c.name.trim() ? c.name.trim() : `${c.broker || 'broker'} / ${c.topic || 'topic'}`;
    return base;
  };

  const saveCurrentToStorage = () => {
    const nm = (config.name || '').trim();
    const finalName = nm || deriveName(config);
    const cleaned: ConfigurationData = { ...config, name: finalName };
    // replace if name exists
    const others = savedConfigs.filter(c => deriveName(c) !== finalName);
    const next = [...others, cleaned];
    setSavedConfigs(next);
    persistSaved(next);
    setConfig(cleaned);
  };

  const handleApply = () => {
    // auto-save on apply when creating new config or edited one
    saveCurrentToStorage();
    onConfigurationSave(config);
    setOpen(false);
  };

  const handleSaveOnly = () => {
    saveCurrentToStorage();
    // Stay on edit mode
  };

  const toBackendConfig = (c: ConfigurationData) => ({
    broker: c.broker,
    topic: c.topic,
    // Backward compatibility flag for older backends; derive from securityType
    ssl_enabled: c.securityType === 'ssl',
    security_type: c.securityType,
    ssl_cert_path: c.sslCertPath || null,
    ssl_key_path: c.sslKeyPath || null,
    ssl_ca_path: c.sslCaPath || null,
    sasl_mechanism: c.saslMechanism || null,
    sasl_jaas_config: c.saslJaasConfig || null,
    message_type: c.messageType,
    proto_schema_path: c.protoSchemaPath || null,
    proto_message_full_name: c.protoSelectedMessage || null,
    proto_descriptor_key: c.protoDescriptorKey || null,
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
        {mode === 'list' ? (
          <div className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle>Saved Configurations</CardTitle>
              </CardHeader>
              <CardContent>
                {savedConfigs.length === 0 ? (
                  <div className="text-sm text-muted-foreground">No saved configurations yet</div>
                ) : (
                  <div className="divide-y rounded border">
                    {savedConfigs
                      .sort((a, b) => deriveName(a).localeCompare(deriveName(b)))
                      .map((c) => {
                        const name = deriveName(c);
                        return (
                          <div key={name} className="flex items-center justify-between px-3 py-2">
                            <div className="flex flex-col">
                              <div className="font-medium">{name}</div>
                              <div className="text-xs text-muted-foreground">{c.broker} / {c.topic}</div>
                            </div>
                            <div className="flex items-center gap-2">
                              <Button
                                variant="secondary"
                                onClick={() => {
                                  setConfig({ ...c });
                                  setProtoFiles(c.protoFiles || []);
                                  setParsedMessages([]);
                                  setParseError(null);
                                  setMode('edit');
                                }}
                              >
                                Edit / Apply
                              </Button>
                              <Button variant="destructive" onClick={() => deleteConfig(name)}>Delete</Button>
                            </div>
                          </div>
                        );
                      })}
                  </div>
                )}
              </CardContent>
            </Card>
            <div className="flex justify-between pt-2">
              <Button variant="outline" onClick={() => setOpen(false)}>Close</Button>
              <Button onClick={startNewConfig}>New Config</Button>
            </div>
          </div>
        ) : (
          <>
            <div className="mb-2">
              <Label htmlFor="conf-name">Name</Label>
              <Input id="conf-name" value={config.name || ''} onChange={(e) => setConfig(prev => ({ ...prev, name: e.target.value }))} placeholder="My Kafka Config" />
            </div>
            <Tabs defaultValue="connection" className="w-full">
              <TabsList className="grid w-full grid-cols-3">
                <TabsTrigger value="connection">Connection</TabsTrigger>
                <TabsTrigger value="security">Security</TabsTrigger>
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
              
              <TabsContent value="security" className="space-y-4">
                <Card>
                  <CardHeader>
                    <CardTitle>Security</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div>
                      <Label htmlFor="security-type">Type</Label>
                      <Select
                        value={config.securityType}
                        onValueChange={(value: 'plaintext' | 'ssl' | 'sasl_plaintext') =>
                          setConfig(prev => ({
                            ...prev,
                            securityType: value,
                            ...(value === 'sasl_plaintext' && (!prev.saslMechanism || prev.saslMechanism.trim() === '')
                              ? { saslMechanism: 'SCRAM-SHA-512' }
                              : {})
                          }))
                        }
                      >
                        <SelectTrigger id="security-type">
                          <SelectValue placeholder="Select security type" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="plaintext">Plaintext</SelectItem>
                          <SelectItem value="ssl">SSL</SelectItem>
                          <SelectItem value="sasl_plaintext">SASL Plaintext</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    {config.securityType === 'ssl' && (
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

                    {config.securityType === 'sasl_plaintext' && (
                      <>
                        <div>
                          <Label htmlFor="sasl-mechanism">SASL Mechanism</Label>
                          <Input
                            id="sasl-mechanism"
                            value={config.saslMechanism || ''}
                            onChange={(e) => setConfig(prev => ({ ...prev, saslMechanism: e.target.value }))}
                            placeholder="SCRAM-SHA-512"
                          />
                        </div>
                        <div>
                          <Label htmlFor="sasl-jaas-config">JAAS Config</Label>
                          <textarea
                            id="sasl-jaas-config"
                            value={config.saslJaasConfig || ''}
                            onChange={(e) => setConfig(prev => ({ ...prev, saslJaasConfig: e.target.value }))}
                            placeholder='org.apache.kafka.common.security.plain.PlainLoginModule required username="user" password="pass";'
                            className="w-full min-h-24 rounded border px-3 py-2 text-sm bg-background"
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
                            onValueChange={(value: string) => setConfig(prev => ({ ...prev, protoSelectedMessage: value }))}
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
              </TabsContent>
            </Tabs>
            
            <div className="flex justify-between space-x-2 pt-4">
              <Button variant="outline" onClick={() => setMode('list')}>
                Back
              </Button>
              <div className="flex gap-2">
                <Button variant="secondary" onClick={handleSaveOnly}>
                  Save
                </Button>
                <Button onClick={handleApply}>
                  Apply
                </Button>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}