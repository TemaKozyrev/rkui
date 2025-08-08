import { useState, useEffect, useMemo } from "react";
import { invoke } from '@tauri-apps/api/core';
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
  protoSchemaPath?: string;
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

  const handlePickProtoFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ filters: [{ name: 'Proto', extensions: ['proto'] }], multiple: false });
      if (typeof selected === 'string') {
        setConfig(prev => ({ ...prev, protoSchemaPath: selected }));
      }
    } catch (e) {
      console.error('Failed to pick proto file', e);
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
      setTopicsError(typeof e === 'string' ? e : (e?.toString?.() || 'Failed to fetch topics'));
      setTopicsBroker(null);
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
                  <div>
                    <Label>Proto Schema File</Label>
                    <div className="flex items-center gap-2">
                      <Button
                        variant="outline"
                        onClick={handlePickProtoFile}
                        className="gap-2"
                      >
                        <Upload className="h-4 w-4" />
                        Select .proto file
                      </Button>
                      {config.protoSchemaPath && (
                        <div className="flex items-center gap-1 text-sm text-muted-foreground">
                          <FileText className="h-4 w-4" />
                          {config.protoSchemaPath}
                        </div>
                      )}
                    </div>
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