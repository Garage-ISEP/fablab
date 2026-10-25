use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::application::dtos::caller::Caller;
use crate::application::errors::AppError;
use crate::application::validation;
use crate::domain::filament::{BambuCredentials, ConnectionState, InventorySnapshot};
use crate::domain::repositories::{
    BambuAccountRepository, BambuCloudApi, MaterialRepository, PrinterTelemetry,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterOverview {
    pub inventory: InventorySnapshot,
    /// When the admin last saved a token (UTC), if ever.
    pub token_saved_at: Option<String>,
}

pub struct GetPrinterOverviewUseCase<A>
where
    A: BambuAccountRepository,
{
    accounts: Arc<A>,
    telemetry: Arc<dyn PrinterTelemetry>,
}

impl<A> GetPrinterOverviewUseCase<A>
where
    A: BambuAccountRepository,
{
    pub fn new(accounts: Arc<A>, telemetry: Arc<dyn PrinterTelemetry>) -> Self {
        Self {
            accounts,
            telemetry,
        }
    }

    pub fn execute(&self, caller: &Caller) -> Result<PrinterOverview, AppError> {
        if !caller.is_admin() {
            return Err(AppError::NotAuthorized);
        }
        Ok(PrinterOverview {
            inventory: self.telemetry.snapshot(),
            token_saved_at: self.accounts.load()?.map(|account| account.updated_at),
        })
    }

    /// Connection status only, cheap enough for a banner on every page.
    pub fn connection(&self) -> ConnectionState {
        self.telemetry.snapshot().connection
    }
}

pub struct ConfigureBambuUseCase<A>
where
    A: BambuAccountRepository,
{
    accounts: Arc<A>,
    api: Arc<dyn BambuCloudApi>,
    telemetry: Arc<dyn PrinterTelemetry>,
}

impl<A> ConfigureBambuUseCase<A>
where
    A: BambuAccountRepository,
{
    pub fn new(
        accounts: Arc<A>,
        api: Arc<dyn BambuCloudApi>,
        telemetry: Arc<dyn PrinterTelemetry>,
    ) -> Self {
        Self {
            accounts,
            api,
            telemetry,
        }
    }

    /// Checks a token pasted by the admin against the Bambu API, saves it
    /// with the account's printers and reconnects. Returns the number of
    /// printers found.
    pub async fn execute(&self, raw_token: &str, caller: &Caller) -> Result<usize, AppError> {
        if !caller.is_admin() {
            return Err(AppError::NotAuthorized);
        }
        let access_token = validation::validate_bambu_token(raw_token)?;

        let user_id = self.api.fetch_user_id(&access_token).await?;
        let devices = self.api.fetch_devices(&access_token).await?;
        if devices.is_empty() {
            return Err(AppError::InvalidInput(
                "no printer bound to this bambu account".to_owned(),
            ));
        }

        let credentials = BambuCredentials {
            user_id,
            access_token,
        };
        self.accounts.save(&credentials, &devices)?;
        let count = devices.len();
        self.telemetry.connect(credentials, devices);
        Ok(count)
    }

    /// Reconnects with the saved token at startup. Returns false when no
    /// token was ever saved.
    pub fn resume(&self) -> Result<bool, AppError> {
        match self.accounts.load()? {
            Some(account) => {
                self.telemetry.connect(account.credentials, account.devices);
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

pub struct SyncMaterialsUseCase<M>
where
    M: MaterialRepository,
{
    materials: Arc<M>,
    telemetry: Arc<dyn PrinterTelemetry>,
}

impl<M> SyncMaterialsUseCase<M>
where
    M: MaterialRepository,
{
    pub fn new(materials: Arc<M>, telemetry: Arc<dyn PrinterTelemetry>) -> Self {
        Self {
            materials,
            telemetry,
        }
    }

    /// Aligns the material list with the AMS content. `synced_revision` is
    /// the value returned by the previous call; nothing is written while
    /// the AMS did not change. Returns the revision to pass next time.
    pub fn execute(&self, synced_revision: Option<u64>) -> Result<Option<u64>, AppError> {
        let inventory = self.telemetry.snapshot();
        if synced_revision == Some(inventory.ams_revision) || inventory.printers.is_empty() {
            return Ok(synced_revision);
        }
        let Some(loaded) = inventory.loaded_materials() else {
            return Ok(synced_revision);
        };

        self.materials.sync_loaded(&loaded)?;
        Ok(Some(inventory.ams_revision))
    }
}
