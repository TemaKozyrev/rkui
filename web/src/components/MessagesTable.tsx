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
}

interface MessagesTableProps {
  messages: KafkaMessage[];
  onMessageClick?: (message: KafkaMessage) => void;
}

export function MessagesTable({ messages, onMessageClick }: MessagesTableProps) {
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
        <div className="border rounded-md">
          <Table>
            <TableHeader>
              <TableRow className="bg-muted/50">
                <TableHead className="w-20">Partition</TableHead>
                <TableHead className="w-32">Key</TableHead>
                <TableHead className="w-24">Offset</TableHead>
                <TableHead className="w-32">Timestamp</TableHead>
                <TableHead>Message</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {messages.map((message) => (
                <TableRow 
                  key={message.id} 
                  className="hover:bg-muted/30 cursor-pointer transition-colors"
                  onClick={() => onMessageClick?.(message)}
                >
                  <TableCell>
                    <Badge variant="outline">{message.partition}</Badge>
                  </TableCell>
                  <TableCell className="font-mono text-sm">
                    {message.key}
                  </TableCell>
                  <TableCell className="font-mono text-sm">
                    {message.offset}
                  </TableCell>
                  <TableCell className="text-sm text-muted-foreground">
                    {message.timestamp}
                  </TableCell>
                  <TableCell>
                    <div className="max-w-md truncate font-mono text-sm">
                      {message.message}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      </CardContent>
    </Card>
  );
}