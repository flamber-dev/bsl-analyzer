"""Contract checks on authored XML, independent of private corpus locations."""

from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest

import metadata_schema as schema


class MetadataSchemaTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.sources = [
            schema.Source("one", "cf", "cf", self.root / "one"),
            schema.Source("two", "ext", "cfe", self.root / "two"),
            schema.Source("tests", "fixture", "fixture", self.root / "fixture"),
        ]
        # Rare is missing in the first configuration and appears twice in the second.
        self.documents = [
            '<Root xmlns="urn:a" xmlns:b="urn:b" version="2.4" b:flag="true">'
            '<Name>SharedToken</Name><b:Name>PrivateName</b:Name></Root>',
            '<x:Root xmlns:x="urn:a" xmlns:y="urn:b" version="2.20">'
            '<x:Name>SharedToken</x:Name><x:Rare>SecretToken</x:Rare>'
            '<x:Rare>SecretToken</x:Rare></x:Root>',
            '<Root xmlns="urn:a" version="2.20"><Name>FixtureToken</Name></Root>',
        ]
        for source, document in zip(self.sources, self.documents):
            source.root.mkdir()
            (source.root / "input.xml").write_text(document, encoding="utf-8")

    def observed(self, jobs=1):
        observation, unassigned = schema.scan(self.sources, [], jobs)
        self.assertEqual(unassigned, [])
        digests = {(f["source"], f["path"]): f["sha256"] for f in observation.files}
        return observation, schema.aggregate(observation.paths, self.sources, digests)

    @staticmethod
    def entry(report, *qnames):
        inverse = {uri: prefix for prefix, uri in report["namespaces"].items()}
        path = "/".join(
            f"{inverse[schema.namespace(q)]}:{schema.local(q)}" if schema.namespace(q) else q
            for q in qnames
        )
        return report["paths"][path]

    def test_qnames_counts_absence_multiplicity_sources_and_versions(self):
        observation, report = self.observed()
        self.assertEqual(len(observation.paths), 4)
        root = self.entry(report, "{urn:a}Root")
        self.assertEqual(root["occ"], 3)
        self.assertEqual(root["docs"], 3)
        self.assertEqual(root["sources"], {"one/cf": 1, "two/ext": 1, "tests/fixture": 1})
        self.assertEqual(root["versions"], {"2.4": 1, "2.20": 2})
        rare_key = next(k for k in root["children"] if k.endswith(":Rare"))
        self.assertEqual(root["children"][rare_key], {"parents": 1, "parents_without": 2, "min": 2, "max": 2})
        rare = self.entry(report, "{urn:a}Root", "{urn:a}Rare")
        self.assertEqual(rare["occ"], 2)
        self.assertEqual(rare["docs"], 1)
        self.assertEqual(rare["sources"], {"two/ext": 2})
        self.assertEqual(rare["versions"], {"2.20": 2})
        self.assertEqual(rare["witness"], ["two/ext", "sha256:" + hashlib.sha256(self.documents[1].encode()).hexdigest()])
        a_name = self.entry(report, "{urn:a}Root", "{urn:a}Name")
        b_name = self.entry(report, "{urn:a}Root", "{urn:b}Name")
        self.assertEqual(a_name["docs"], 3)
        self.assertEqual(b_name["docs"], 1)
        self.assertEqual(b_name["text"]["withheld"], 1)
        flag = next(v for k, v in root["attributes"].items() if k.endswith(":flag"))
        self.assertEqual(flag["values"], {"true": {"occ": 1, "sources": 1}})
        self.assertEqual(observation.paths[("{urn:a}Root", "{urn:a}Rare")].source_docs, {"two/ext": 1})

    def test_values_are_observations_and_private_names_stay_local(self):
        _, report = self.observed()
        name = self.entry(report, "{urn:a}Root", "{urn:a}Name")["text"]
        self.assertEqual(name["values"], {
            "SharedToken": {"occ": 2, "sources": 2},
            "FixtureToken": {"occ": 1, "sources": 1},
        })
        self.assertTrue(name["domain"].startswith("observed"))
        rare = self.entry(report, "{urn:a}Root", "{urn:a}Rare")["text"]
        self.assertEqual(rare["values"], {})
        self.assertEqual(rare["withheld"], 1)
        public_text = json.dumps(report)
        self.assertNotIn("SecretToken", public_text)
        self.assertNotIn("PrivateName", public_text)
        self.assertNotIn("minOccurs", public_text)
        self.assertNotIn("enum", name)

    def test_prefix_spelling_is_not_part_of_the_key(self):
        left = schema.Observation()
        right = schema.Observation()
        left.scan_document(0, "tests/fixture", "a.xml", b'<a:R xmlns:a="urn:r"><a:V>true</a:V></a:R>')
        right.scan_document(0, "tests/fixture", "a.xml", b'<b:R xmlns:b="urn:r"><b:V>true</b:V></b:R>')
        self.assertEqual(schema.aggregate(left.paths, self.sources, None), schema.aggregate(right.paths, self.sources, None))

    def test_broken_xml_is_in_manifest_but_never_a_partial_observation(self):
        for number, data in enumerate([b"<Root><Partial/>", b"<Root><unknown:Name/></Root>"]):
            (self.sources[0].root / f"broken{number}.xml").write_bytes(data)
        observation, report = self.observed()
        self.assertEqual(len(observation.files), 5)
        self.assertEqual(len(observation.errors), 2)
        self.assertEqual(len(observation.paths), 4)
        self.assertEqual(self.entry(report, "{urn:a}Root")["docs"], 3)
        manifest = schema.manifest(observation, self.sources, [], private=False)
        first = manifest["sources"][0]
        self.assertEqual(first["documents"], 3)
        self.assertEqual(first["roots"]["!error version=-"], 2)
        self.assertEqual(len(first["parse_errors"]), 2)
        for error in first["parse_errors"]:
            self.assertNotIn("path", error)
            self.assertEqual(len(error["sha256"]), 64)
            self.assertTrue(error["error"])
        local = schema.manifest(observation, self.sources, [], private=True)["sources"][0]
        self.assertEqual({e["path"] for e in local["parse_errors"]}, {"broken0.xml", "broken1.xml"})
        self.assertEqual(len(local["files"]), 3)

    def test_parallel_chunk_merge_and_source_order_reproduce_the_aggregate(self):
        one, serial = self.observed(jobs=1)
        two, parallel = self.observed(jobs=2)
        self.assertEqual(serial, parallel)
        self.assertEqual(one.files, two.files)
        self.assertEqual(one.errors, two.errors)
        reversed_obs, _ = schema.scan(list(reversed(self.sources)), [], jobs=1)
        digests = {(f["source"], f["path"]): f["sha256"] for f in reversed_obs.files}
        self.assertEqual(serial, schema.aggregate(reversed_obs.paths, self.sources, digests))
        for name, report in [("a.json", serial), ("b.json", parallel)]:
            schema.write_aggregate(self.root / name, report)
        self.assertEqual((self.root / "a.json").read_bytes(), (self.root / "b.json").read_bytes())
        self.assertEqual(json.loads((self.root / "a.json").read_text()), serial)

    def test_fixture_manifest_has_relative_paths_and_exact_hashes(self):
        observation, _ = self.observed()
        manifest = schema.manifest(observation, self.sources, [], private=False)
        fixture = manifest["sources"][2]
        file_hash = hashlib.sha256(self.documents[2].encode()).hexdigest()
        self.assertEqual(fixture["files"], [{"path": "input.xml", "sha256": file_hash}])
        self.assertEqual(fixture["files_sha256"], hashlib.sha256(f"input.xml\0{file_hash}\n".encode()).hexdigest())
        self.assertEqual(fixture["kind"], "fixture")
        self.assertEqual(fixture["roots"], {"Root version=2.20": 1})
        for private in manifest["sources"][:2]:
            self.assertNotIn("files", private)
            self.assertNotIn("locator", private)

    def test_nested_sources_and_extensions_have_separate_ids_without_double_counting(self):
        base = self.root / "extensions"
        for name in ["B", "A"]:
            path = base / name
            path.mkdir(parents=True)
            (path / "Configuration.xml").write_text("<Root/>")
        (base / "unassigned.xml").write_text("<Root/>")
        extensions, returned_base = schema.expand_extensions(f"project={base}")
        self.assertEqual(returned_base, base)
        self.assertEqual([(s.id, s.kind, s.label) for s in extensions], [
            ("project/cfe-1", "cfe", "A"), ("project/cfe-2", "cfe", "B")
        ])
        observation, unassigned = schema.scan(extensions, [base], 1)
        self.assertEqual(len(observation.files), 2)
        self.assertEqual(unassigned, [{"path": str(base / "unassigned.xml")}])
        parent = schema.Source("project", "parent", "cf", base)
        nested, _ = schema.scan([parent, *extensions], [], 1)
        self.assertEqual(len(nested.files), 3)
        self.assertEqual(sum(f["source"] == parent.id for f in nested.files), 1)

    def test_layout_distinguishes_declared_kind_and_observed_directory(self):
        observation = schema.Observation()
        ns = schema.MD_CLASSES
        observation.scan_document(0, "one/cf", "Configuration.xml", (
            f'<MetaDataObject xmlns="{ns}"><Configuration><ChildObjects>'
            '<Catalog>A</Catalog><Interface>B</Interface></ChildObjects></Configuration></MetaDataObject>'
        ).encode())
        observation.scan_document(1, "one/cf", "Catalogs/A.xml", (
            f'<MetaDataObject xmlns="{ns}"><Catalog><Properties><Name>A</Name></Properties></Catalog></MetaDataObject>'
        ).encode())
        layout = schema.layout_report(observation)
        self.assertEqual(layout["Catalog"], {"directories": {"Catalogs": {"one/cf": 1}}, "declared_in": {"one/cf": 1}})
        self.assertEqual(layout["Interface"], {"directories": {}, "declared_in": {"one/cf": 1}})

    def test_repeated_parent_paths_count_documents_once_and_observe_variable_multiplicity(self):
        observation = schema.Observation()
        observation.scan_document(0, "tests/fixture", "rows.xml", (
            b'<Root><Row><Field>true</Field></Row>'
            b'<Row><Field>false</Field><Field>true</Field></Row><Row/></Root>'
        ))
        report = schema.aggregate(observation.paths, self.sources, None)
        row = self.entry(report, "Root", "Row")
        self.assertEqual(row["occ"], 3)
        self.assertEqual(row["docs"], 1)
        self.assertEqual(row["children"]["Field"], {"parents": 2, "parents_without": 1, "min": 1, "max": 2})
        self.assertEqual(row["witness"], ["tests/fixture", "rows.xml"])

    def test_supplementary_mdo_comparison_uses_names_and_values_only(self):
        _, report = self.observed()
        aggregate = self.root / "aggregate.json"
        schema.write_aggregate(aggregate, report)
        mdo = self.root / "mdo"
        mdo.mkdir()
        (mdo / "item.mdo").write_text(
            '<DifferentRoot><name>SharedToken</name><name>MdoOnly</name></DifferentRoot>'
        )
        with redirect_stdout(io.StringIO()) as output:
            schema.main(["crosscheck-mdo", "--aggregate", str(aggregate), "--mdo-root", str(mdo)])
        result = json.loads(output.getvalue())
        self.assertEqual(result["mdo_files"], 1)
        self.assertEqual(result["shared_names"], 1)
        self.assertEqual(result["values"]["name"], {
            "both": ["SharedToken"], "xml_only": ["FixtureToken"],
            "mdo_only_count": 1, "xml_withheld": 1,
        })
        self.assertNotIn("MdoOnly", output.getvalue())

    def test_duplicate_source_id_or_root_is_rejected_before_publication(self):
        first = self.sources[0]
        cases = [
            [f"{first.id}=cf:{first.root}", f"{first.id}=cf:{self.sources[1].root}"],
            [f"{first.id}=cf:{first.root}", f"other/cf=cf:{first.root}"],
        ]
        for specs in cases:
            arguments = ["scan", "--out", str(self.root / "rejected")]
            for spec in specs:
                arguments += ["--source", spec]
            with self.assertRaises(SystemExit):
                schema.main(arguments)
            self.assertFalse((self.root / "rejected").exists())

    def test_cli_writes_public_and_local_outputs_reproducibly(self):
        arguments = ["scan", "--out", str(self.root / "public"), "--local-out", str(self.root / "local")]
        for source in self.sources:
            arguments += ["--source", f"{source.id}={source.kind}:{source.root}"]
        with redirect_stdout(io.StringIO()) as output:
            schema.main(arguments)
        self.assertIn("sources=3 documents=3 paths=4 published_paths=4 errors=0", output.getvalue())
        published = (self.root / "public/aggregate.json").read_bytes()
        local_values = (self.root / "local/values.local.json").read_text()
        self.assertIn("SecretToken", local_values)
        self.assertNotIn(b"SecretToken", published)
        with redirect_stdout(io.StringIO()):
            schema.main(arguments)
        self.assertEqual((self.root / "public/aggregate.json").read_bytes(), published)
        for file in ["manifest.json", "layout.json"]:
            self.assertIsInstance(json.loads((self.root / "public" / file).read_text()), dict)


if __name__ == "__main__":
    unittest.main()
