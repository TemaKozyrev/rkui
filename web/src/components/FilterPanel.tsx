import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Button } from "./ui/button";
import { RefreshCw } from "lucide-react";

interface FilterPanelProps {
  onFilterChange: (filters: any) => void;
  onRefresh: () => void;
  currentConfig?: any;
}

export function FilterPanel({ onFilterChange, onRefresh, currentConfig }: FilterPanelProps) {
  return (
    <div className="w-80 bg-muted/30 p-6 space-y-4">
      <Card>
        <CardHeader>
          <CardTitle>Current Topic</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-sm text-muted-foreground">
            <div><strong>Broker:</strong> {currentConfig?.broker || 'Not configured'}</div>
            <div><strong>Topic:</strong> {currentConfig?.topic || 'Not configured'}</div>
            <div><strong>Type:</strong> {currentConfig?.messageType?.toUpperCase() || 'Unknown'}</div>
            {currentConfig?.sslEnabled && (
              <div><strong>SSL:</strong> Enabled</div>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Quick Filters</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div>
            <Label htmlFor="partition-filter">Partition</Label>
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
          </div>
          
          <div>
            <Label htmlFor="key-filter">Key Filter</Label>
            <Input 
              id="key-filter" 
              placeholder="Filter by key"
              onChange={(e) => onFilterChange({ keyFilter: e.target.value })}
            />
          </div>
          
          <div>
            <Label htmlFor="message-filter">Message Filter</Label>
            <Input 
              id="message-filter" 
              placeholder="Filter by content"
              onChange={(e) => onFilterChange({ messageFilter: e.target.value })}
            />
          </div>
        </CardContent>
      </Card>

      <Button className="w-full gap-2" onClick={onRefresh}>
        <RefreshCw className="h-4 w-4" />
        Refresh Messages
      </Button>
    </div>
  );
}