import { Card, CardContent } from "./ui/card";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Input } from "./ui/input";
import { Button } from "./ui/button";
import { Upload } from "lucide-react";

interface ConfigSecurityShape {
  securityType: 'plaintext' | 'ssl' | 'sasl_plaintext' | 'sasl_ssl';
  sslMode?: 'java_like' | 'classic';
  truststoreLocation?: string;
  truststorePassword?: string;
  sslCaRoot?: string;
  sslCaSub?: string;
  sslCertificate?: string;
  sslKey?: string;
  sslKeyPassword?: string;
  saslMechanism?: string;
  saslJaasConfig?: string;
}

interface Props {
  config: ConfigSecurityShape;
  setConfig: React.Dispatch<React.SetStateAction<any>>;
  handlePickTruststore: () => Promise<void>;
  handlePickCaRoot: () => Promise<void>;
  handlePickCaSub: () => Promise<void>;
  handlePickCertificate: () => Promise<void>;
  handlePickKey: () => Promise<void>;
}

export default function ConfigurationSecurity({
  config,
  setConfig,
  handlePickTruststore,
  handlePickCaRoot,
  handlePickCaSub,
  handlePickCertificate,
  handlePickKey,
}: Props) {
  return (
    <Card>
      <CardContent className="pt-2 space-y-4">
        <div>
          <Label htmlFor="security-type">Type</Label>
          <Select
            value={config.securityType}
            onValueChange={(value: 'plaintext' | 'ssl' | 'sasl_plaintext' | 'sasl_ssl') =>
              setConfig((prev: any) => ({
                ...prev,
                securityType: value,
                ...(((value === 'sasl_plaintext' || value === 'sasl_ssl') && (!prev.saslMechanism || prev.saslMechanism.trim() === ''))
                  ? { saslMechanism: 'SCRAM-SHA-512' }
                  : {})
              }))
            }
          >
            <SelectTrigger id="security-type">
              <SelectValue placeholder="Select security type" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="plaintext">Plaintext</SelectItem>
              <SelectItem value="ssl">SSL</SelectItem>
              <SelectItem value="sasl_plaintext">SASL Plaintext</SelectItem>
              <SelectItem value="sasl_ssl">SASL SSL</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {(config.securityType === 'ssl' || config.securityType === 'sasl_ssl') && (
          <>
            <div>
              <Label htmlFor="ssl-mode">SSL Mode</Label>
              <Select
                value={config.sslMode || 'java_like'}
                onValueChange={(value: 'java_like' | 'classic') => setConfig((prev: any) => ({ ...prev, sslMode: value }))}
              >
                <SelectTrigger id="ssl-mode">
                  <SelectValue placeholder="Select SSL mode" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="java_like">Java-like (Truststore)</SelectItem>
                  <SelectItem value="classic">Classic SSL (PEM files)</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {(!config.sslMode || config.sslMode === 'java_like') && (
              <>
                <div className="pt-2">
                  <Label>Truststore / CA</Label>
                  <div className="flex items-center gap-2">
                    <Button variant="outline" onClick={handlePickTruststore} className="gap-2">
                      <Upload className="h-4 w-4" />
                      Select truststore/CA
                    </Button>
                    {config.truststoreLocation && (
                      <Button variant="ghost" size="sm" onClick={() => setConfig((prev: any) => ({ ...prev, truststoreLocation: undefined }))}>
                        Clear
                      </Button>
                    )}
                  </div>
                  <div className="mt-1 text-xs text-muted-foreground">
                    Supported: .pem, .crt, .cer, .bundle, .jks, .jceks, .p12, .pfx. Note: .jks may be either legacy JKS/JCEKS or PKCS#12; the app will detect the format automatically.
                  </div>
                  {config.truststoreLocation && (
                    <div className="mt-1 text-xs text-muted-foreground break-all" title={config.truststoreLocation}>
                      {config.truststoreLocation}
                    </div>
                  )}
                </div>
                <div>
                  <Label htmlFor="truststore-password">Truststore Password</Label>
                  <Input
                    id="truststore-password"
                    type="password"
                    value={config.truststorePassword || ''}
                    onChange={(e) => setConfig((prev: any) => ({ ...prev, truststorePassword: e.target.value }))}
                    placeholder="optional"
                  />
                </div>
              </>
            )}

            {config.sslMode === 'classic' && (
              <>
                <div className="pt-2">
                  <Label>CA Root</Label>
                  <div className="flex items-center gap-2">
                    <Button variant="outline" onClick={handlePickCaRoot} className="gap-2">
                      <Upload className="h-4 w-4" />
                      Select CA Root (.pem/.crt)
                    </Button>
                    {config.sslCaRoot && (
                      <Button variant="ghost" size="sm" onClick={() => setConfig((prev: any) => ({ ...prev, sslCaRoot: undefined }))}>
                        Clear
                      </Button>
                    )}
                  </div>
                  {config.sslCaRoot && (
                    <div className="mt-1 text-xs text-muted-foreground break-all" title={config.sslCaRoot}>
                      {config.sslCaRoot}
                    </div>
                  )}
                </div>

                <div>
                  <Label>CA Intermediate (optional)</Label>
                  <div className="flex items-center gap-2">
                    <Button variant="outline" onClick={handlePickCaSub} className="gap-2">
                      <Upload className="h-4 w-4" />
                      Select CA Sub (.pem/.crt)
                    </Button>
                    {config.sslCaSub && (
                      <Button variant="ghost" size="sm" onClick={() => setConfig((prev: any) => ({ ...prev, sslCaSub: undefined }))}>
                        Clear
                      </Button>
                    )}
                  </div>
                  {config.sslCaSub && (
                    <div className="mt-1 text-xs text-muted-foreground break-all" title={config.sslCaSub}>
                      {config.sslCaSub}
                    </div>
                  )}
                </div>

                <div>
                  <Label>Client Certificate</Label>
                  <div className="flex items-center gap-2">
                    <Button variant="outline" onClick={handlePickCertificate} className="gap-2">
                      <Upload className="h-4 w-4" />
                      Select certificate (.pem/.crt)
                    </Button>
                    {config.sslCertificate && (
                      <Button variant="ghost" size="sm" onClick={() => setConfig((prev: any) => ({ ...prev, sslCertificate: undefined }))}>
                        Clear
                      </Button>
                    )}
                  </div>
                  {config.sslCertificate && (
                    <div className="mt-1 text-xs text-muted-foreground break-all" title={config.sslCertificate}>
                      {config.sslCertificate}
                    </div>
                  )}
                </div>

                <div>
                  <Label>Client Private Key</Label>
                  <div className="flex items-center gap-2">
                    <Button variant="outline" onClick={handlePickKey} className="gap-2">
                      <Upload className="h-4 w-4" />
                      Select key (.key/.pem)
                    </Button>
                    {config.sslKey && (
                      <Button variant="ghost" size="sm" onClick={() => setConfig((prev: any) => ({ ...prev, sslKey: undefined }))}>
                        Clear
                      </Button>
                    )}
                  </div>
                  {config.sslKey && (
                    <div className="mt-1 text-xs text-muted-foreground break-all" title={config.sslKey}>
                      {config.sslKey}
                    </div>
                  )}
                </div>

                <div>
                  <Label htmlFor="ssl-key-password">Key Password (optional)</Label>
                  <Input
                    id="ssl-key-password"
                    type="password"
                    value={config.sslKeyPassword || ''}
                    onChange={(e) => setConfig((prev: any) => ({ ...prev, sslKeyPassword: e.target.value }))}
                    placeholder="optional"
                  />
                </div>
              </>
            )}
          </>
        )}

        {(config.securityType === 'sasl_plaintext' || config.securityType === 'sasl_ssl') && (
          <>
            <div>
              <Label htmlFor="sasl-mechanism">SASL Mechanism</Label>
              <Input
                id="sasl-mechanism"
                value={config.saslMechanism || ''}
                onChange={(e) => setConfig((prev: any) => ({ ...prev, saslMechanism: e.target.value }))}
                placeholder="SCRAM-SHA-512"
              />
            </div>
            <div>
              <Label htmlFor="sasl-jaas-config">JAAS Config</Label>
              <textarea
                id="sasl-jaas-config"
                value={config.saslJaasConfig || ''}
                onChange={(e) => {
                  const normalized = e.target.value.replace(/[«»]/g, '"');
                  setConfig((prev: any) => ({ ...prev, saslJaasConfig: normalized }));
                }}
                placeholder='org.apache.kafka.common.security.plain.PlainLoginModule required username="user" password="pass";'
                className="w-full min-h-24 rounded border px-3 py-2 text-sm bg-background"
                spellCheck={false}
                autoCorrect="off"
                autoCapitalize="off"
              />
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
