package main

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/gin-contrib/cors"
	ginfw "github.com/gin-gonic/gin"
	x402 "github.com/x402-foundation/x402/go"
	x402http "github.com/x402-foundation/x402/go/http"
	ginmwx402 "github.com/x402-foundation/x402/go/http/gin"

	casperServer "github.com/make-software/casper-x402/go/x402/mechanisms/casper/exact/server"

	"github.com/make-software/casper-x402/go/internal/logger"
	"github.com/make-software/casper-x402/go/internal/logger/ginmw"
)

// fatal logs an error at the error level and exits with code 1.
func fatal(msg string, kv ...any) {
	logger.Default().Error(msg, kv...)
	os.Exit(1)
}

type staticAuthProvider struct {
	apiKey string
}

func (p *staticAuthProvider) GetAuthHeaders(_ context.Context) (x402http.AuthHeaders, error) {
	h := map[string]string{"Authorization": p.apiKey}
	return x402http.AuthHeaders{
		Verify:    h,
		Settle:    h,
		Supported: h,
	}, nil
}

func main() {
	var cfg Env
	if err := cfg.Parse(); err != nil {
		panic(fmt.Sprintf("Error parsing configuration: %v", err))
	}

	log := logger.Default()

	assetName := cfg.AssetName
	assetPackage := strings.Replace(cfg.AssetPackage, "hash-", "", -1)
	x402Network := x402.Network(cfg.ChainID)

	log.Info("server starting",
		"asset_package", assetPackage,
		"payee_address", cfg.PayeeAddress,
		"network", x402Network,
		"facilitator_url", cfg.FacilitatorURL,
		"port", cfg.Port,
	)

	ginfw.DebugPrintFunc = func(format string, values ...any) {
		log.Debug(fmt.Sprintf(format, values...), logger.FieldType, "gin")
	}

	r := ginfw.New()
	r.Use(ginmw.RequestID(log))
	r.Use(ginmw.AccessLog())
	r.Use(ginfw.Recovery())
	r.Use(cors.New(cors.Config{
		AllowAllOrigins: true,
		AllowMethods:    []string{http.MethodGet, http.MethodPost, http.MethodOptions},
		AllowHeaders:    []string{"Accept", "Authorization", "Content-Type", "Origin", "Payment-Signature"},
		ExposeHeaders:   []string{"PAYMENT-REQUIRED", "PAYMENT-RESPONSE"},
		MaxAge:          24 * time.Hour,
	}))

	facilitatorCfg := &x402http.FacilitatorConfig{
		URL: cfg.FacilitatorURL,
	}

	if cfg.FacilitatorAPIKey != "" {
		facilitatorCfg.AuthProvider = &staticAuthProvider{apiKey: cfg.FacilitatorAPIKey}
	}

	facilitatorClient := x402http.NewHTTPFacilitatorClient(facilitatorCfg)

	routes := x402http.RoutesConfig{
		"GET /weather": {
			Accepts: x402http.PaymentOptions{
				{
					Scheme:  "exact",
					Price:   "$0.001",
					Network: x402Network,
					PayTo:   cfg.PayeeAddress,
				},
			},
			Description: "Get weather data for a city",
			MimeType:    "application/json",
		},
	}

	casperScheme := casperServer.NewExactCasperScheme().
		RegisterMoneyParser(func(amount float64, network x402.Network) (*x402.AssetAmount, error) {
			return &x402.AssetAmount{
				Amount: fmt.Sprintf("%d", 7_500000000),
				Asset:  assetPackage,
				Extra:  map[string]interface{}{"name": assetName, "version": "1", "decimals": "9"},
			}, nil
		}).
		RegisterAsset(cfg.ChainID, assetPackage, 2)

	// Create x402 resource server with hooks
	server := x402.Newx402ResourceServer(
		x402.WithFacilitatorClient(facilitatorClient),
	).
		Register(x402Network, casperScheme).
		// Hook 1: Before Verify - Called before payment verification
		// Can abort verification by returning &BeforeHookResult{Abort: true, Reason: "..."}
		OnBeforeVerify(func(ctx x402.VerifyContext) (*x402.BeforeHookResult, error) {
			logger.Ctx(ctx.Ctx).Info("Before verify hook - validating payment requirements",
				"scheme", ctx.Requirements.GetScheme(),
				"network", ctx.Requirements.GetNetwork())
			logger.Ctx(ctx.Ctx).Debug("Payment details",
				"requirements_json", string(ctx.RequirementsBytes),
				"payload_json", string(ctx.PayloadBytes))
			// Example: Abort verification
			// return &x402.BeforeHookResult{Abort: true, Reason: "Custom validation failed"}, nil
			return nil, nil
		}).
		// Hook 2: After Verify - Called after successful payment verification
		OnAfterVerify(func(ctx x402.VerifyResultContext) error {
			logger.Ctx(ctx.Ctx).Info("After verify hook", "IsValid", ctx.Result.IsValid)
			if !ctx.Result.IsValid {
				logger.Ctx(ctx.Ctx).Warn("Payment verification failed", "reason", ctx.Result.InvalidReason)
			}
			return nil
		}).
		// Hook 3: Verify Failure - Called when payment verification fails
		// Can recover from failure by returning &VerifyFailureHookResult{Recovered: true, Result: ...}
		OnVerifyFailure(func(ctx x402.VerifyFailureContext) (*x402.VerifyFailureHookResult, error) {
			logger.Ctx(ctx.Ctx).Error("Verify failure hook", "error", ctx.Error)
			// Example: Recover from failure
			// return &x402.VerifyFailureHookResult{
			// 	Recovered: true,
			// 	Result:    &x402.VerifyResponse{IsValid: true, InvalidReason: "Recovered from failure"},
			// }, nil
			return nil, nil
		}).
		// Hook 4: Before Settle - Called before payment settlement
		// Can abort settlement by returning &BeforeHookResult{Abort: true, Reason: "..."}
		OnBeforeSettle(func(ctx x402.SettleContext) (*x402.BeforeHookResult, error) {
			logger.Ctx(ctx.Ctx).Info("Before settle hook",
				"scheme", ctx.Requirements.GetScheme(),
				"network", ctx.Requirements.GetNetwork())
			// Example: Abort settlement
			// return &x402.BeforeHookResult{Abort: true, Reason: "Settlement temporarily disabled"}, nil
			return nil, nil
		}).
		// Hook 5: After Settle - Called after successful payment settlement
		OnAfterSettle(func(ctx x402.SettleResultContext) error {
			logger.Ctx(ctx.Ctx).Info("After settle hook",
				"Success", ctx.Result.Success,
				"Transaction", ctx.Result.Transaction)
			if ctx.Result.Success {
				agentID := ctx.Result.Payer
				amount := ctx.Result.Amount
				if amount == "" {
					amount = "7500000000" // raw X402 units (9 decimals) for the $0.001 route
				}
				home, _ := os.UserHomeDir()
				cli := os.Getenv("PAYROLL_CLI")
				if cli == "" {
					cli = filepath.Join(home, "pv-target/release/payroll_vault_cli")
				}
				dir := os.Getenv("PAYROLL_DIR")
				if dir == "" {
					dir = "/mnt/c/Users/User/Downloads/payroll-vault/payroll-vault"
				}
				go func(agentID, amount string) {
					cmd := exec.Command(cli,
						"contract", "PayrollVault", "record_payment",
						"--agent_id", agentID,
						"--amount", amount,
						"--gas", "5 cspr")
					cmd.Dir = dir
					out, err := cmd.CombinedOutput()
					if err != nil {
						logger.Default().Error("payrollvault record failed", "err", err, "output", string(out))
						return
					}
					logger.Default().Info("payrollvault payment recorded", "agent_id", agentID, "amount", amount)
				}(agentID, amount)
			}
			return nil
		}).
		// Hook 6: Settle Failure - Called when payment settlement fails
		// Can recover from failure by returning &SettleFailureHookResult{Recovered: true, Result: ...}
		OnSettleFailure(func(ctx x402.SettleFailureContext) (*x402.SettleFailureHookResult, error) {
			logger.Ctx(ctx.Ctx).Error("Settle failure hook", "error", ctx.Error)
			// Example: Recover from failure
			// return &x402.SettleFailureHookResult{
			// 	Recovered: true,
			// 	Result:    &x402.SettleResponse{Success: true, Transaction: "0x123..."},
			// }, nil
			return nil, nil
		})

	r.Use(ginmwx402.PaymentMiddleware(routes, server))

	r.GET("/weather", func(c *ginfw.Context) {
		city := c.DefaultQuery("city", "Barcelona")
		weatherData := map[string]map[string]interface{}{
			"Barcelona":     {"weather": "sunny", "temperature": 45},
			"San Francisco": {"weather": "foggy", "temperature": 60},
			"New York":      {"weather": "cloudy", "temperature": 55},
			"London":        {"weather": "rainy", "temperature": 50},
			"Tokyo":         {"weather": "clear", "temperature": 65},
		}
		data, exists := weatherData[city]
		if !exists {
			data = map[string]interface{}{"weather": "sunny", "temperature": 70}
		}
		c.JSON(http.StatusOK, ginfw.H{
			"city":        city,
			"weather":     data["weather"],
			"temperature": data["temperature"],
			"timestamp":   time.Now().Format(time.RFC3339),
		})
	})

	r.GET("/health", func(c *ginfw.Context) {
		c.JSON(http.StatusOK, ginfw.H{"status": "ok", "version": "2.0.0"})
	})

	if err := r.Run(fmt.Sprintf(":%d", cfg.Port)); err != nil {
		fatal("server error", "error", err.Error())
	}
}
