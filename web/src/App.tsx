import { useState } from "react";
import { ConfigurationModal } from "./components/ConfigurationModal";
import { FilterPanel } from "./components/FilterPanel";
import { MessagesTable } from "./components/MessagesTable";
import { MessagesPagination } from "./components/MessagesPagination";
import { MessageDetailModal } from "./components/MessageDetailModal";
import { Button } from "./components/ui/button";
import { Play, Pause } from "lucide-react";
import { invoke } from '@tauri-apps/api/core';

// Мок данные для демонстрации
const generateMockMessages = (page: number, pageSize: number = 10, messageType: string = 'json') => {
  const messages = [];
  const startIndex = (page - 1) * pageSize;
  
  for (let i = 0; i < pageSize; i++) {
    const messageIndex = startIndex + i;
    let messageContent = '';
    
    switch (messageType) {
      case 'json':
        messageContent = JSON.stringify({
          event: ['login', 'logout', 'purchase', 'view_product'][Math.floor(Math.random() * 4)],
          userId: Math.floor(Math.random() * 10000),
          timestamp: new Date().toISOString(),
          data: { sessionId: `sess-${Math.floor(Math.random() * 1000)}` }
        });
        break;
      case 'text':
        messageContent = `User ${Math.floor(Math.random() * 1000)} performed action at ${new Date().toISOString()}`;
        break;
      case 'protobuf':
        messageContent = `[binary protobuf data - ${Math.floor(Math.random() * 1000)} bytes]`;
        break;
    }
    
    messages.push({
      id: `msg-${messageIndex}`,
      partition: Math.floor(Math.random() * 3),
      key: `user-${Math.floor(Math.random() * 1000)}`,
      offset: 1000 + messageIndex,
      message: messageContent,
      timestamp: new Date(Date.now() - Math.random() * 86400000).toLocaleString()
    });
  }
  
  return messages;
};

export default function App() {
  const [currentPage, setCurrentPage] = useState(1);
  const [messages, setMessages] = useState(() => generateMockMessages(1));
  const [isConnected, setIsConnected] = useState(false);
  const [currentConfig, setCurrentConfig] = useState<any>(null);
  const [selectedMessage, setSelectedMessage] = useState<any>(null);
  const [messageDetailOpen, setMessageDetailOpen] = useState(false);
  const totalPages = 50; // Симуляция большого количества страниц

  const handlePageChange = (page: number) => {
    setCurrentPage(page);
    const messageType = currentConfig?.messageType || 'json';
    setMessages(generateMockMessages(page, 10, messageType));
  };


  const handleConfigurationSave = async (config: any) => {
    console.log('Configuration saved (UI will try to configure backend):', config);
    // Transform to backend snake_case
    const payload = {
      broker: config.broker,
      topic: config.topic,
      ssl_enabled: config.sslEnabled,
      ssl_cert_path: config.sslCertPath || null,
      ssl_key_path: config.sslKeyPath || null,
      ssl_ca_path: config.sslCaPath || null,
      message_type: config.messageType,
      partition: config.partition,
      offset_type: config.offsetType,
      start_offset: config.startOffset,
      proto_schema_path: config.protoSchemaPath || null,
    };

    try {
      await invoke('set_kafka_config', { config: payload });
      setCurrentConfig(config);
      setIsConnected(true);
      setMessages(generateMockMessages(1, 10, config.messageType));
      setCurrentPage(1);
    } catch (e) {
      console.error('Failed to configure Kafka backend:', e);
      setIsConnected(false);
    }
  };

  const handleFilterChange = (filters: any) => {
    console.log('Filters changed:', filters);
    // Здесь можно добавить логику фильтрации
  };

  const handleRefresh = () => {
    const messageType = currentConfig?.messageType || 'json';
    setMessages(generateMockMessages(currentPage, 10, messageType));
  };

  const toggleConnection = () => {
    setIsConnected(!isConnected);
  };

  const handleMessageClick = (message: any) => {
    setSelectedMessage(message);
    setMessageDetailOpen(true);
  };

  return (
    <div className="h-screen bg-background flex flex-col">
      {/* Header */}
      <div className="border-b px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-4">
          <h1>Kafka Message Viewer</h1>
          <ConfigurationModal onConfigurationSave={handleConfigurationSave} />
        </div>
        
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`} />
          <span className="text-sm text-muted-foreground">
            {isConnected ? 'Connected' : 'Disconnected'}
          </span>
          {currentConfig && (
            <Button
              variant="outline"
              size="sm"
              onClick={toggleConnection}
              className="gap-2"
            >
              {isConnected ? (
                <>
                  <Pause className="h-4 w-4" />
                  Pause
                </>
              ) : (
                <>
                  <Play className="h-4 w-4" />
                  Start
                </>
              )}
            </Button>
          )}
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex">
        <FilterPanel 
          onFilterChange={handleFilterChange}
          onRefresh={handleRefresh}
          currentConfig={currentConfig}
        />
        
        <div className="flex-1 flex flex-col">
          <div className="flex-1 p-6">
            {currentConfig ? (
              <MessagesTable 
                messages={messages} 
                onMessageClick={handleMessageClick}
              />
            ) : (
              <div className="flex items-center justify-center h-full">
                <div className="text-center">
                  <h2>Welcome to Kafka Message Viewer</h2>
                  <p className="text-muted-foreground mt-2">
                    Click "Configure" to set up your Kafka connection
                  </p>
                </div>
              </div>
            )}
          </div>
          
          {currentConfig && (
            <MessagesPagination
              currentPage={currentPage}
              totalPages={totalPages}
              onPageChange={handlePageChange}
            />
          )}
        </div>
      </div>

      {/* Message Detail Modal */}
      <MessageDetailModal
        message={selectedMessage}
        open={messageDetailOpen}
        onOpenChange={setMessageDetailOpen}
        messageType={currentConfig?.messageType || 'json'}
      />
    </div>
  );
}