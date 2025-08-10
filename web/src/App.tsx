import { useState } from "react";
import { ConfigurationModal } from "./components/ConfigurationModal";
import { FilterPanel } from "./components/FilterPanel";
import { MessagesTable } from "./components/MessagesTable";
import { MessagesPagination } from "./components/MessagesPagination";
import { MessageDetailModal } from "./components/MessageDetailModal";
import { Button } from "./components/ui/button";
import { Play, Pause } from "lucide-react";
import { invoke } from '@tauri-apps/api/core';
import { message as showDialog } from '@tauri-apps/plugin-dialog';

type KafkaMessage = {
  id: string;
  partition: number;
  key: string;
  offset: number;
  message: string;
  timestamp: string;
  decoding_error?: string; // backend snake_case
  decodingError?: string; // in case it comes as camelCase
};

const PAGE_SIZE = 20;

export default function App() {
  const [currentPage, setCurrentPage] = useState(1);
  const [buffer, setBuffer] = useState<KafkaMessage[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [currentConfig, setCurrentConfig] = useState<any>(null);
  const [selectedMessage, setSelectedMessage] = useState<any>(null);
  const [messageDetailOpen, setMessageDetailOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [partitions, setPartitions] = useState<number[]>([]);
  const [appliedFilters, setAppliedFilters] = useState<{ partition: string; startOffset: number; startFrom: 'oldest' | 'newest' }>({
    partition: 'all',
    startOffset: 0,
    startFrom: 'oldest',
  });
  const [pendingFilters, setPendingFilters] = useState<{ partition: string; startOffset: number; startFrom: 'oldest' | 'newest' }>({
    partition: 'all',
    startOffset: 0,
    startFrom: 'oldest',
  });

  const totalPages = Math.max(1, Math.ceil(buffer.length / PAGE_SIZE));
  const pageMessages = buffer.slice((currentPage - 1) * PAGE_SIZE, currentPage * PAGE_SIZE);

  const fetchNextBatch = async () => {
    setIsLoading(true);
    try {
      const next: KafkaMessage[] = await invoke('consume_next_messages', { limit: 200 });
      if (Array.isArray(next) && next.length > 0) {
        setBuffer(prev => [...prev, ...next]);
        // If any messages failed to decode, notify the user
        const failed = next.filter(m => (m.decoding_error || m.decodingError));
        if (failed.length > 0) {
          const first = failed[0];
          const errText = (first.decoding_error || first.decodingError) as string;
          await showDialog(`Некоторые сообщения не удалось декодировать. Показан текстовый формат.\nПервая ошибка: ${errText}`, { title: 'Ошибка декодирования Protobuf', kind: 'error' });
        }
      }
    } catch (e: any) {
      console.error('Failed to fetch messages:', e);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to fetch messages');
      await showDialog(text, { title: 'Fetch Messages Error', kind: 'error' });
    } finally {
      setIsLoading(false);
    }
  };

  const handlePageChange = async (page: number) => {
    setCurrentPage(page);
    // If navigating to the last page and it is full, prefetch next 150
    const isLastPage = page === Math.max(1, Math.ceil(buffer.length / PAGE_SIZE));
    const thisPageLen = buffer.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE).length;
    if (isLastPage && thisPageLen === PAGE_SIZE) {
      fetchNextBatch();
    }
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
      proto_schema_path: config.protoSchemaPath || null,
      proto_message_full_name: config.protoSelectedMessage || null,
    };

    try {
      await invoke('set_kafka_config', { config: payload });
      // Fetch partitions for this topic
      try {
        const parts: number[] = await invoke('get_topic_partitions', { config: payload });
        setPartitions(parts || []);
      } catch (e: any) {
        console.warn('Failed to fetch partitions for topic', e);
        setPartitions([]);
        const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to fetch partitions for topic');
        await showDialog(text, { title: 'Partitions Error', kind: 'error' });
      }
      setCurrentConfig(config);
      setIsConnected(true);
      setBuffer([]);
      setCurrentPage(1);
      // Reset filters to defaults on new configuration
      const defaults: { partition: string; startOffset: number; startFrom: 'oldest' | 'newest' } = { partition: 'all', startOffset: 0, startFrom: 'oldest' };
      setAppliedFilters(defaults);
      setPendingFilters(defaults);
      await fetchNextBatch();
    } catch (e: any) {
      console.error('Failed to configure Kafka backend:', e);
      setIsConnected(false);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to configure Kafka backend');
      await showDialog(text, { title: 'Configuration Error', kind: 'error' });
    }
  };

  const handleFilterChange = (changed: any) => {
    const next = { ...pendingFilters, ...changed };
    setPendingFilters(next);
  };

  const handleRefresh = async () => {
    if (!currentConfig) return;
    // Apply pending filters if they differ from applied
    const isDirty = JSON.stringify(pendingFilters) !== JSON.stringify(appliedFilters);
    try {
      if (isDirty) {
        await invoke('apply_filters', {
          args: {
            partition: pendingFilters.partition,
            start_offset: pendingFilters.startOffset,
            start_from: pendingFilters.startFrom,
          },
        });
        setAppliedFilters(pendingFilters);
      }
    } catch (e: any) {
      console.error('Failed to apply filters on refresh', e);
      const text = typeof e === 'string' ? e : (e?.toString?.() || 'Failed to apply filters');
      await showDialog(text, { title: 'Apply Filters Error', kind: 'error' });
    }
    // Reset local buffer and fetch fresh batch from current consumer position
    setBuffer([]);
    setCurrentPage(1);
    await fetchNextBatch();
  };

  const toggleConnection = () => {
    setIsConnected(!isConnected);
  };

  const handleMessageClick = (message: any) => {
    setSelectedMessage(message);
    setMessageDetailOpen(true);
  };

  const isDirty = JSON.stringify(pendingFilters) !== JSON.stringify(appliedFilters);

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
        {currentConfig && isConnected && (
          <FilterPanel 
            onFilterChange={handleFilterChange}
            onRefresh={handleRefresh}
            currentConfig={currentConfig}
            partitions={partitions}
            refreshDisabled={!isDirty}
            selectedPartition={pendingFilters.partition}
            selectedStartFrom={pendingFilters.startFrom}
          />
        )}
        
        <div className="flex-1 flex flex-col">
          <div className="flex-1 p-6">
            {currentConfig ? (
              <MessagesTable 
                messages={pageMessages} 
                onMessageClick={handleMessageClick}
                loading={isLoading}
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