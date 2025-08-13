import { useState, useEffect, useMemo } from "react";
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog, message as showDialog } from '@tauri-apps/plugin-dialog';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Settings } from "lucide-react";
import ConfigurationConnection from "./ConfigurationConnection";
import ConfigurationSecurity from "./ConfigurationSecurity";
import ConfigurationMessages from "./ConfigurationMessages";

interface ConfigurationData {
  name?: string; // UI-only: human-friendly name for saved configs
  broker: string;
  topic: string;
  securityType: 'plaintext' | 'ssl' | 'sasl_plaintext' | 'sasl_ssl';
  // SSL Mode and Classic SSL fields
  sslMode?: 'java_like' | 'classic';
  sslCaRoot?: string;
  sslCaSub?: string;
  sslCertificate?: string;
  sslKey?: string;
  sslKeyPassword?: string;
  // SASL/SSL advanced stores
  truststoreLocation?: string;
  truststorePassword?: string;
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

  const handleBrokerChange = (val: string) => {
    setConfig(prev => ({ ...prev, broker: val }));
    if (topicsBroker && val !== topicsBroker) {
      setTopics([]);
      setTopicsBroker(null);
      setTopicsError(null);
    }
  };

  // Proto parsing UI state
  const [protoFiles, setProtoFiles] = useState<string[]>([]);
  const [isParsingProto, setIsParsingProto] = useState(false);
  const [parsedMessages, setParsedMessages] = useState<string[]>([]);
  const [parseError, setParseError] = useState<string | null>(null);

  const handlePickTruststore = async () => {
    try {
      const selected = await openDialog({
        multiple: false,
        filters: [{ name: 'Truststore/CA', extensions: ['pem','crt','cer','bundle','jks','jceks','p12','pfx'] }]
      });
      if (!selected) return;
      const path = Array.isArray(selected) ? (typeof selected[0] === 'string' ? selected[0] : null) : (typeof selected === 'string' ? selected : null);
      if (!path) return;
      try {
            const internal: string = await invoke('import_app_file', { srcPath: path, kind: 'truststore' });
            setConfig(prev => ({ ...prev, truststoreLocation: internal }));
          } catch (err: any) {
            console.error('Failed to import truststore into app', err);
            await showDialog((err?.toString?.() || 'Failed to import truststore'), { title: 'Import Error', kind: 'error' });
          }
    } catch (e: any) {
      console.error('Failed to pick truststore file', e);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to open file dialog');
      await showDialog(text, { title: 'Error', kind: 'error' });
    }
  };

  const pickAndImport = async (filters: { name: string; extensions: string[] }, kind: string): Promise<string | null> => {
    try {
      const selected = await openDialog({ multiple: false, filters: [filters] });
      if (!selected) return null;
      const path = Array.isArray(selected) ? (typeof selected[0] === 'string' ? selected[0] : null) : (typeof selected === 'string' ? selected : null);
      if (!path) return null;
      try {
        const internal: string = await invoke('import_app_file', { srcPath: path, kind });
        return internal;
      } catch (err: any) {
        console.error('Failed to import file into app', err);
        // Fallback to original path on import failure
        return path;
      }
    } catch (e: any) {
      console.error('Failed to pick file', e);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to open file dialog');
      await showDialog(text, { title: 'Error', kind: 'error' });
      return null;
    }
  };

  const handlePickCaRoot = async () => {
    const res = await pickAndImport({ name: 'CA Root', extensions: ['pem','crt','cer','bundle'] }, 'truststore');
    if (res) setConfig(prev => ({ ...prev, sslMode: prev.sslMode || 'classic', sslCaRoot: res }));
  };
  const handlePickCaSub = async () => {
    const res = await pickAndImport({ name: 'CA Intermediate', extensions: ['pem','crt','cer','bundle'] }, 'truststore');
    if (res) setConfig(prev => ({ ...prev, sslMode: prev.sslMode || 'classic', sslCaSub: res }));
  };
  const handlePickCertificate = async () => {
    const res = await pickAndImport({ name: 'Certificate', extensions: ['pem','crt','cer'] }, 'truststore');
    if (res) setConfig(prev => ({ ...prev, sslMode: prev.sslMode || 'classic', sslCertificate: res }));
  };
  const handlePickKey = async () => {
    const res = await pickAndImport({ name: 'Private Key', extensions: ['key','pem'] }, 'truststore');
    if (res) setConfig(prev => ({ ...prev, sslMode: prev.sslMode || 'classic', sslKey: res }));
  };


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
      // Import into app-managed storage and merge with existing
      const imported = await Promise.all(
        incoming.map(async (p) => {
          try {
            const internal: string = await invoke('import_app_file', { srcPath: p, kind: 'proto' });
            return internal;
          } catch (err: any) {
            console.error('Failed to import proto file into app', p, err);
            return p; // fallback to original path if import fails
          }
        })
      );
      const mergedSet = new Set<string>(protoFiles);
      for (const f of imported) { mergedSet.add(f); }
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
    ssl_enabled: c.securityType === 'ssl' || c.securityType === 'sasl_ssl',
    security_type: c.securityType,
    // SSL mode and Classic SSL fields
    ssl_mode: c.sslMode || null,
    ssl_ca_root: c.sslCaRoot || null,
    ssl_ca_sub: c.sslCaSub || null,
    ssl_certificate: c.sslCertificate || null,
    ssl_key: c.sslKey || null,
    ssl_key_password: c.sslKeyPassword || null,
    // Advanced truststore options
    truststore_location: c.truststoreLocation || null,
    truststore_password: c.truststorePassword || null,
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
                <ConfigurationConnection
                  config={config}
                  setConfig={setConfig}
                  isFetchingTopics={isFetchingTopics}
                  topicsError={topicsError}
                  filteredTopics={filteredTopics}
                  handleBrokerBlur={handleBrokerBlur}
                  handleBrokerChange={handleBrokerChange}
                />
              </TabsContent>
              
              <TabsContent value="security" className="space-y-4">
                <ConfigurationSecurity
                  config={config as any}
                  setConfig={setConfig}
                  handlePickTruststore={handlePickTruststore}
                  handlePickCaRoot={handlePickCaRoot}
                  handlePickCaSub={handlePickCaSub}
                  handlePickCertificate={handlePickCertificate}
                  handlePickKey={handlePickKey}
                />
              </TabsContent>
              
              <TabsContent value="messages" className="space-y-4">
                <ConfigurationMessages
                  config={config as any}
                  setConfig={setConfig}
                  protoFiles={protoFiles}
                  isParsingProto={isParsingProto}
                  parsedMessages={parsedMessages}
                  parseError={parseError}
                  handlePickProtoFile={handlePickProtoFile}
                  handleLoadProtoMetadata={handleLoadProtoMetadata}
                  handleCleanProto={handleCleanProto}
                />
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