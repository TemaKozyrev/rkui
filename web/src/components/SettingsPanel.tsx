import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface SettingsPanelProps {
  onConfigChange: (config: any) => void;
}

export function SettingsPanel({ onConfigChange }: SettingsPanelProps) {
  return (
    <div className="w-80 bg-muted/30 p-6 space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>Configure</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <Label htmlFor="broker">Kafka Broker</Label>
            <Input 
              id="broker" 
              placeholder="localhost:9092" 
              defaultValue="localhost:9092"
            />
          </div>
          <div>
            <Label htmlFor="topic">Topic</Label>
            <Input 
              id="topic" 
              placeholder="my-topic"
              defaultValue="user-events"
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Partition</CardTitle>
        </CardHeader>
        <CardContent>
          <Select defaultValue="all">
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
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Offset From</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <Label htmlFor="offset">Start Offset</Label>
            <Input 
              id="offset" 
              type="number" 
              placeholder="0" 
              defaultValue="0"
            />
          </div>
          <Select defaultValue="latest">
            <SelectTrigger>
              <SelectValue placeholder="Offset type" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="earliest">Earliest</SelectItem>
              <SelectItem value="latest">Latest</SelectItem>
              <SelectItem value="custom">Custom</SelectItem>
            </SelectContent>
          </Select>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Filter</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <Label htmlFor="keyFilter">Key Filter</Label>
            <Input 
              id="keyFilter" 
              placeholder="Filter by key"
            />
          </div>
          <div>
            <Label htmlFor="messageFilter">Message Filter</Label>
            <Input 
              id="messageFilter" 
              placeholder="Filter by message content"
            />
          </div>
        </CardContent>
      </Card>

      <Button className="w-full" onClick={() => onConfigChange({})}>
        Apply Configuration
      </Button>
    </div>
  );
}