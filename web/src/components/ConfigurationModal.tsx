import { useState } from "react";
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
  protoSchema?: File;
  partition: string;
  offsetType: string;
  startOffset: number;
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
    messageType: 'json',
    partition: 'all',
    offsetType: 'latest',
    startOffset: 0
  });

  const handleFileUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) {
      setConfig(prev => ({ ...prev, protoSchema: file }));
    }
  };

  const handleSave = () => {
    onConfigurationSave(config);
    setOpen(false);
  };

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
                    onChange={(e) => setConfig(prev => ({ ...prev, broker: e.target.value }))}
                    placeholder="localhost:9092"
                  />
                </div>
                <div>
                  <Label htmlFor="topic">Topic</Label>
                  <Input
                    id="topic"
                    value={config.topic}
                    onChange={(e) => setConfig(prev => ({ ...prev, topic: e.target.value }))}
                    placeholder="my-topic"
                  />
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
                    onCheckedChange={(checked) => setConfig(prev => ({ ...prev, sslEnabled: checked }))}
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
                    <Label htmlFor="proto-schema">Proto Schema File</Label>
                    <div className="flex items-center gap-2">
                      <Input
                        id="proto-schema"
                        type="file"
                        accept=".proto"
                        onChange={handleFileUpload}
                        className="hidden"
                      />
                      <Button
                        variant="outline"
                        onClick={() => document.getElementById('proto-schema')?.click()}
                        className="gap-2"
                      >
                        <Upload className="h-4 w-4" />
                        Upload .proto file
                      </Button>
                      {config.protoSchema && (
                        <div className="flex items-center gap-1 text-sm text-muted-foreground">
                          <FileText className="h-4 w-4" />
                          {config.protoSchema.name}
                        </div>
                      )}
                    </div>
                  </div>
                )}
                
                <div>
                  <Label htmlFor="partition">Partition</Label>
                  <Select
                    value={config.partition}
                    onValueChange={(value) => setConfig(prev => ({ ...prev, partition: value }))}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Select partition" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">All partitions</SelectItem>
                      <SelectItem value="0">Partition 0</SelectItem>
                      <SelectItem value="1">Partition 1</SelectItem>
                      <SelectItem value="2">Partition 2</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <Label htmlFor="offset-type">Offset Type</Label>
                    <Select
                      value={config.offsetType}
                      onValueChange={(value) => setConfig(prev => ({ ...prev, offsetType: value }))}
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="Offset type" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="earliest">Earliest</SelectItem>
                        <SelectItem value="latest">Latest</SelectItem>
                        <SelectItem value="custom">Custom</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <div>
                    <Label htmlFor="start-offset">Start Offset</Label>
                    <Input
                      id="start-offset"
                      type="number"
                      value={config.startOffset}
                      onChange={(e) => setConfig(prev => ({ ...prev, startOffset: parseInt(e.target.value) || 0 }))}
                      placeholder="0"
                    />
                  </div>
                </div>
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