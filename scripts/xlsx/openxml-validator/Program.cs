using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;
using System.Security.Cryptography;
using System.Text.Json;

if (args.Length != 2) throw new ArgumentException("Expected corpus directory and report path");
var validator = new OpenXmlValidator(FileFormatVersions.Office2021) { MaxNumberOfErrors = 1000 };
var records = new List<object>();
int failures = 0;
foreach (var path in Directory.GetFiles(args[0], "*.xlsx").OrderBy(p => p)) {
    var before = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path)));
    using (var document = SpreadsheetDocument.Open(path, false)) {
        var errors = validator.Validate(document).Select(e => new {
            id = e.Id, description = e.Description, type = e.ErrorType.ToString(),
            part = e.Part?.Uri.ToString(), xpath = e.Path?.XPath
        }).ToArray();
        records.Add(new { file = Path.GetFileName(path), sha256 = before, errors });
        failures += errors.Length;
        Console.WriteLine($"{Path.GetFileName(path)}: {errors.Length} errors");
        foreach (var error in errors.Take(5)) Console.WriteLine($"{error.part} {error.xpath}: {error.description}");
    }
    if (before != Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(path)))) throw new Exception("Read-only validator changed a workbook");
}
var report = new { validator = "DocumentFormat.OpenXml", package_version = "3.5.1", assembly_version = typeof(SpreadsheetDocument).Assembly.GetName().Version?.ToString(), file_format = "Office2021", read_only = true, errors = failures, workbooks = records, scope = "Actual SDK package/schema/semantic-constraint validation; not Excel application refresh, rendering or save/repair validation" };
File.WriteAllText(args[1], JsonSerializer.Serialize(report, new JsonSerializerOptions { WriteIndented = true }));
if (failures != 0) Environment.Exit(1);
