import {invoke} from "@tauri-apps/api/core";
import type {ProviderSetupBridge} from "./provider-setup-types";
export const nativeProviderSetup:ProviderSetupBridge={
 providerSetupStatuses:()=>invoke("provider_setup_statuses"),
 saveProviderCredential:(expected,input)=>invoke("save_provider_credential",{expected,input}),
 testProviderCredential:(operationId,expected)=>invoke("test_provider_credential",{operationId,expected}),
 removeProviderCredential:provider=>invoke("remove_provider_credential",{provider}),
};
