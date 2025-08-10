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
  partitions?: number[];
  refreshDisabled?: boolean;
  selectedPartition?: string;
  selectedStartFrom?: 'oldest' | 'newest';
}

export function FilterPanel({ onFilterChange, onRefresh, currentConfig, partitions = [], refreshDisabled = false, selectedPartition = 'all', selectedStartFrom = 'oldest' }: FilterPanelProps) {
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
          <CardTitle>Start From</CardTitle>
        </CardHeader>
        <CardContent>
          <div>
            <Label htmlFor="start-from-select">Start position</Label>
            <Select value={selectedStartFrom} onValueChange={(value) => onFilterChange({ startFrom: value as 'oldest' | 'newest' })}>
              <SelectTrigger>
                <SelectValue placeholder="Select start position" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="oldest">Oldest</SelectItem>
                <SelectItem value="newest">Newest</SelectItem>
              </SelectContent>
            </Select>
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
            <Select defaultValue="all" onValueChange={(value) => onFilterChange({ partition: value })}>
              <SelectTrigger>
                <SelectValue placeholder="Select partition" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All partitions</SelectItem>
                {(partitions ?? []).map((p) => (
                  <SelectItem key={p} value={String(p)}>{`Partition ${p}`}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {selectedPartition !== 'all' && (
            <div>
              <Label htmlFor="start-offset">Start Offset</Label>
              <Input
                id="start-offset"
                type="number"
                placeholder="0"
                onChange={(e) => onFilterChange({ startOffset: parseInt(e.target.value || '0', 10) })}
              />
            </div>
          )}
          
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

      <Button className="w-full gap-2" onClick={onRefresh} disabled={refreshDisabled}>
        <RefreshCw className="h-4 w-4" />
        Refresh Messages
      </Button>
    </div>
  );
}