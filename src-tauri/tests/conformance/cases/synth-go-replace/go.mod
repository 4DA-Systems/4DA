module example.com/synth-go-replace

go 1.21

require (
	github.com/gin-gonic/gin v1.6.0
	golang.org/x/net v0.0.0-20220722155237-a158d28d115b
	golang.org/x/text v0.3.7
	gopkg.in/yaml.v2 v2.2.2
)

require (
	golang.org/x/mod v0.6.0-dev.0.20220419223038-86c51ed26bb4 // indirect
	golang.org/x/sys v0.0.0-20220722155257-8c9f86f7a55f // indirect
	golang.org/x/term v0.0.0-20210927222741-03fcf44c2211 // indirect
	golang.org/x/tools v0.1.12 // indirect
	gopkg.in/check.v1 v0.0.0-20161208181325-20d25e280405 // indirect
)

// A version pin: the build list installs x/text v0.3.8, not the v0.3.7 the require line names.
replace golang.org/x/text => golang.org/x/text v0.3.8

// A local checkout: nothing is installed from the registry for gin.
replace github.com/gin-gonic/gin => ./third_party/gin
