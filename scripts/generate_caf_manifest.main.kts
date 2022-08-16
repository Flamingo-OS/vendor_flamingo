#!/home2/npv12/kotlinc/bin/kotlin

@file:Repository("https://repo.maven.apache.org/maven2/")
@file:DependsOn("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.1")
@file:Import("util.main.kts")

import javax.net.ssl.HttpsURLConnection
import javax.xml.parsers.DocumentBuilderFactory
import javax.xml.transform.OutputKeys
import javax.xml.transform.TransformerFactory
import javax.xml.transform.dom.DOMSource
import javax.xml.transform.stream.StreamResult

import java.io.File
import java.net.URL

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking

import kotlin.system.exitProcess

import org.w3c.dom.Element
import org.w3c.dom.Node

private val CLO_URL = "https://git.codelinaro.org/clo/la"
private val SYSTEM_MANIFEST_BASE_URL = "$CLO_URL/la/system/manifest/-/raw"
private val VENDOR_MANIFEST_BASE_URL = "$CLO_URL/la/vendor/manifest/-/raw"

private val elementsToKeep = listOf(
    ManifestElement.PROJECT
)

private val attributesToKeep = listOf(
    ManifestAttr.CLONE_DEPTH,
    ManifestAttr.NAME,
    ManifestAttr.PATH
)

private val shallowCloneRepos = listOf(
    "platform/external/*",
    "platform/prebuilts/*"
).map {
    it.toRegex()
}

private lateinit var manifestDir: String
private var systemTag: String? = null
private var vendorTag: String? = null

parseArgs()
fetchManifests()

fun parseArgs() {
    if (args.contains("-h")) {
        help()
        exitProcess(0)
    }
    manifestDir = Utils.getArgValue(Args.MANIFEST_DIR, args)
    if (args.contains(Args.SYSTEM_TAG)) {
        systemTag = Utils.getArgValue(Args.SYSTEM_TAG, args)
    }
    if (args.contains(Args.VENDOR_TAG)) {
        vendorTag = Utils.getArgValue(Args.VENDOR_TAG, args)
    }
    if (systemTag == null && vendorTag == null) {
        Log.info("Neither system nor vendor tag provided, exiting")
        exitProcess(0)
    }
}

fun help() {
    println("Script to download and reformat system and vendor manifest from codelinaro\n" +
            "Usage: ./generate_caf_manifest.main.kts -dir [Directory to create manifest files in]\n" +
                                                    "-system-tag [Tag of system manifest]\n" +
                                                    "-vendor-tag [Tag of vendor manifest]"
    )
}

fun fetchManifests() {
    runBlocking(Dispatchers.IO) {
        systemTag?.let {
            launch {
                fetchManifest(
                    SYSTEM_MANIFEST_BASE_URL,
                    it,
                    File(manifestDir, Manifests.SYSTEM),
                    Remotes.SYSTEM
                )
                Log.info("Generated system manifest successfully")
            }
        }
        vendorTag?.let {
            launch {
                fetchManifest(
                    VENDOR_MANIFEST_BASE_URL,
                    it,
                    File(manifestDir, Manifests.VENDOR),
                    Remotes.VENDOR
                )
                Log.info("Generated vendor manifest successfully")
            }
        }
    }
}

fun fetchManifest(baseUrl: String, tag: String, outputFile: File, remote: String) {
    // Parse document
    val docBuilder = DocumentBuilderFactory.newInstance().newDocumentBuilder()
    val url = URL("$baseUrl/$tag/$tag.xml")
    val connection = url.openConnection() as HttpsURLConnection
    val doc = connection.inputStream.use {
        docBuilder.parse(it)
    }

    val rootElement = doc.getElementsByTagName(ManifestElement.ROOT).item(0) as Element
    val nodeList = rootElement.childNodes
    var i = 0
    while (i < nodeList.length) {
        val node = nodeList.item(i)
        val newNode = node.cloneNode(true /* deep */).apply { textContent = null }

        // Remove any free spaces
        if (node.nodeType != Node.ELEMENT_NODE) {
            rootElement.replaceChild(newNode, node)
            i++
            continue
        }

        // Remove unnecessary elements
        if (!elementsToKeep.contains(node.nodeName)) {
            rootElement.removeChild(node)
            continue
        }
        val newElement = newNode as Element

        // Remove attributes we don't need
        val attrs = node.attributes
        for (j in 0 until attrs.length) {
            val attr = attrs.item(j).nodeName
            if (!attributesToKeep.contains(attr)) {
                newElement.removeAttribute(attr)
            }
        }

        // Set remote attribute
        newElement.setAttribute(ManifestAttr.REMOTE, remote)

        // Set clone depth
        val projectName = newElement.getAttribute(ManifestAttr.NAME)
        val shouldSetCloneDepth = shallowCloneRepos.any { projectName.contains(it) }
        if (shouldSetCloneDepth && !newElement.hasAttribute(ManifestAttr.CLONE_DEPTH)) {
            newElement.setAttribute(ManifestAttr.CLONE_DEPTH, "1")
        }
        rootElement.replaceChild(newElement, node)
        i++
    }

    // Write doc to file
    val transformer = TransformerFactory.newInstance().newTransformer().apply {
        setOutputProperty(OutputKeys.INDENT, "yes")
    }
    val domSource = DOMSource(doc)
    outputFile.outputStream().use {
        transformer.transform(domSource, StreamResult(it))
        it.flush()
    }
}

private object Args {
    const val MANIFEST_DIR = "-dir"
    const val SYSTEM_TAG = "-system-tag"
    const val VENDOR_TAG = "-vendor-tag"
}

private object ManifestElement {
    const val ROOT = "manifest"
    const val PROJECT = "project"
}

private object ManifestAttr {
    const val CLONE_DEPTH = "clone-depth"
    const val NAME = "name"
    const val PATH = "path"
    const val REMOTE = "remote"
}

private object Manifests {
    const val SYSTEM = "system.xml"
    const val VENDOR = "vendor.xml"
}

private object Remotes {
    const val SYSTEM = "clo_system"
    const val VENDOR = "clo_vendor"
}
