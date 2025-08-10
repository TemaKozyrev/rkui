import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "./ui/table";
import { Badge } from "./ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface KafkaMessage {
  id: string;
  partition: number;
  key: string;
  offset: number;
  message: string;
  timestamp: string;
  decoding_error?: string;
  decodingError?: string;
}

import { Loader2 } from "lucide-react";

interface MessagesTableProps {
  messages: KafkaMessage[];
  onMessageClick?: (message: KafkaMessage) => void;
  loading?: boolean;
}

export function MessagesTable({ messages, onMessageClick, loading = false }: MessagesTableProps) {
  const showLoader = loading && messages.length === 0;

  return (
    <Card className="flex-1">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          Kafka Messages
          <span className="text-sm text-muted-foreground font-normal">
            (Click on a message to view details)
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {showLoader ? (
          <div className="h-64 flex items-center justify-center">
            <div className="flex flex-col items-center gap-3 text-muted-foreground">
              <Loader2 className="h-8 w-8 animate-spin" />
              <span className="text-sm">Loading messages...</span>
            </div>
          </div>
        ) : (
          <div className="border rounded-md">
            <Table className="table-fixed w-full">
              <TableHeader>
                <TableRow className="bg-muted/50">
                  <TableHead className="w-20">Partition</TableHead>
                  <TableHead className="w-32">Key</TableHead>
                  <TableHead className="w-24">Offset</TableHead>
                  <TableHead className="w-[30ch] whitespace-nowrap">Timestamp</TableHead>
                  <TableHead>Message</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {messages.map((message) => {
                  const hasError = !!(message.decoding_error || message.decodingError);
                  return (
                    <TableRow 
                      key={message.id} 
                      className={`${hasError ? 'bg-red-50/70' : ''} hover:bg-muted/30 cursor-pointer transition-colors`}
                      onClick={() => onMessageClick?.(message)}
                    >
                      <TableCell>
                        <Badge variant="outline">{message.partition}</Badge>
                      </TableCell>
                      <TableCell className="font-mono text-sm">
                        <div className="w-32 truncate whitespace-nowrap" title={message.key}>
                          {message.key}
                        </div>
                      </TableCell>
                      <TableCell className="font-mono text-sm">
                        {message.offset}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground whitespace-nowrap w-[30ch] font-mono">
                        {message.timestamp}
                      </TableCell>
                      <TableCell>
                        <div className="font-mono text-sm truncate whitespace-nowrap">
                          {message.message}
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}
      </CardContent>
    </Card>
  );
}